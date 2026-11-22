pluginManagement {
    repositories {
        gradlePluginPortal()
        google()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "TimeLeft"

// Shared Compose UI lives in ui/src (plain sources compiled into each app module).
include(":desktop")
// Later: include(":app") (Android phone), include(":wear") (Wear OS).
