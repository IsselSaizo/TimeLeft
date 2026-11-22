@echo off
rem Time Left desktop preview: builds the Rust core if needed and opens the window.
rem Shares state with the CLI: %APPDATA%\TimeLeft\state.json (override with TIMELEFT_STATE).
setlocal
if not defined JAVA_HOME (
  if exist "F:\Android\Android Studio\jbr\bin\java.exe" set "JAVA_HOME=F:\Android\Android Studio\jbr"
)
if not defined JAVA_HOME (
  if exist "%ProgramFiles%\Android\Android Studio\jbr\bin\java.exe" set "JAVA_HOME=%ProgramFiles%\Android\Android Studio\jbr"
)
if not defined JAVA_HOME (
  echo JAVA_HOME is not set and Android Studio's JBR was not found. Java 25 is required.
  exit /b 1
)
rem Keep Gradle caches off the system drive when F:\Android exists.
if not defined GRADLE_USER_HOME (
  if exist "F:\Android\.gradle" set "GRADLE_USER_HOME=F:\Android\.gradle"
)
cd /d "%~dp0android"
call gradlew.bat --console=plain -q :desktop:run
