import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    kotlin("jvm")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

// ---- Rust core via UniFFI ---------------------------------------------------------------
// cargo builds timeleft-ffi as a native library; uniffi-bindgen generates the Kotlin API
// from it; the library is put on the classpath where JNA looks for it.

val repoRoot: File = rootDir.parentFile
val cargoRelease: File = repoRoot.resolve("target/release")
val osName = System.getProperty("os.name").lowercase()
val osArch = System.getProperty("os.arch").lowercase()
val nativeLib = when {
    osName.contains("win") -> "timeleft_ffi.dll"
    osName.contains("mac") -> "libtimeleft_ffi.dylib"
    else -> "libtimeleft_ffi.so"
}
val jnaPrefix = when {
    osName.contains("win") -> "win32-x86-64"
    osName.contains("mac") -> if (osArch == "aarch64") "darwin-aarch64" else "darwin-x86-64"
    else -> if (osArch == "aarch64") "linux-aarch64" else "linux-x86-64"
}
val uniffiKotlinDir = layout.buildDirectory.dir("generated/uniffi/kotlin")
val uniffiNativeDir = layout.buildDirectory.dir("generated/uniffi/native")

val cargoBuildFfi = tasks.register<Exec>("cargoBuildFfi") {
    group = "rust"
    description = "cargo build -p timeleft-ffi --release"
    workingDir = repoRoot
    commandLine("cargo", "build", "-p", "timeleft-ffi", "--release")
    inputs.files(
        fileTree(repoRoot.resolve("core/src")),
        fileTree(repoRoot.resolve("ffi/src")),
        repoRoot.resolve("Cargo.toml"),
        repoRoot.resolve("Cargo.lock"),
        repoRoot.resolve("core/Cargo.toml"),
        repoRoot.resolve("ffi/Cargo.toml"),
        repoRoot.resolve("data/lifetable.bin"),
    )
    outputs.file(cargoRelease.resolve(nativeLib))
}

val generateUniffiKotlin = tasks.register<Exec>("generateUniffiKotlin") {
    group = "rust"
    description = "Generate Kotlin bindings with uniffi-bindgen"
    dependsOn(cargoBuildFfi)
    val outDir = uniffiKotlinDir.get().asFile
    workingDir = repoRoot
    commandLine(
        "cargo", "run", "-q", "-p", "uniffi-bindgen", "--",
        "generate", "--library", cargoRelease.resolve(nativeLib).path,
        "--language", "kotlin", "--out-dir", outDir.path, "--no-format",
    )
    inputs.file(cargoRelease.resolve(nativeLib))
    inputs.file(repoRoot.resolve("ffi/uniffi.toml"))
    outputs.dir(outDir)
    doFirst { outDir.deleteRecursively() }
}

val copyNativeLib = tasks.register<Copy>("copyNativeLib") {
    group = "rust"
    dependsOn(cargoBuildFfi)
    from(cargoRelease.resolve(nativeLib))
    into(uniffiNativeDir.map { it.dir(jnaPrefix) })
}

kotlin {
    jvmToolchain(25)
    compilerOptions {
        optIn.add("androidx.compose.material3.ExperimentalMaterial3Api")
    }
    sourceSets.named("main") {
        kotlin.srcDir("../ui/src")
        kotlin.srcDir(uniffiKotlinDir.get().asFile)
    }
}

sourceSets.named("main") {
    resources.srcDir(uniffiNativeDir.get().asFile)
}

tasks.named("compileKotlin") { dependsOn(generateUniffiKotlin) }
tasks.named("processResources") { dependsOn(copyNativeLib) }

dependencies {
    implementation(compose.desktop.currentOs)
    implementation("org.jetbrains.compose.material3:material3:1.9.0")
    implementation("net.java.dev.jna:jna:5.19.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-swing:1.11.0")
}

// Headless renders of all screens: gradlew :desktop:screenshots -Pstate=path\to\state.json
tasks.register<JavaExec>("screenshots") {
    group = "verification"
    description = "Render every screen to PNG (build/screenshots)"
    mainClass = "app.timeleft.desktop.ScreenshotsKt"
    classpath = sourceSets["main"].runtimeClasspath
    val state = providers.gradleProperty("state")
    val out = layout.buildDirectory.dir("screenshots").get().asFile.path
    argumentProviders.add(CommandLineArgumentProvider { listOf(state.get(), out) })
}

compose.desktop {
    application {
        mainClass = "app.timeleft.desktop.MainKt"
        // Skia and JNA load native code; silence the Java 24+ restricted-method warning.
        jvmArgs += "--enable-native-access=ALL-UNNAMED"
        nativeDistributions {
            targetFormats(TargetFormat.Msi)
            packageName = "TimeLeft"
            packageVersion = "0.1.0"
            description = "Time Left — lifetime countdown"
        }
    }
}
