; rza shell context menu — per-user (HKCU), cascading "rza" submenu.
;
; This file defines NSIS macros that register a cascading right-click
; context-menu entry ("rza") on Windows for common archive file types,
; plus a cascading "rza" submenu on all files and folders for
; compression actions.
;
; Usage:
;   !include "shell-menu.nsh"
;   ; In install section:  !insertmacro RzaInstallShellMenu
;   ; In uninstall section: !insertmacro RzaUninstallShellMenu
;
; NOTE: cargo-packager 0.11.8 does NOT support !include of external .nsh
; files or an uninstall hook.  The install-side registry writes are wired
; via the [package.metadata.packager.nsis] preinstall-section key in
; Cargo.toml.  The uninstall-side cleanup requires either:
;   (a) a future cargo-packager version with an uninstall hook, or
;   (b) a fully custom NSIS template (nsis.template key).
; This file is kept as the canonical reference for both macros.

!macro RzaInstallShellMenu
  ; --- Archive types: cascading "rza" submenu with extract/test/open actions ---

  ; .zip
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; .tar
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; .gz
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; .tgz
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; .bz2
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; .xz
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; .zst
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; .7z
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; .rar
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'

  ; --- All files: cascading "rza" submenu with compress actions ---
  WriteRegStr HKCU "Software\Classes\*\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\*\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\01add" "" "Add to archive…"
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\01add\command" "" '"$INSTDIR\rza-gui.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\02zip" "" "Compress to .zip"
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\02zip\command" "" '"$INSTDIR\rza.exe" compress-zip "%1"'
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\03targz" "" "Compress to .tar.gz"
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\03targz\command" "" '"$INSTDIR\rza.exe" compress-targz "%1"'

  ; --- Directories: cascading "rza" submenu with compress actions ---
  WriteRegStr HKCU "Software\Classes\Directory\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\Directory\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\Directory\shell\rza\shell\01add" "" "Add to archive…"
  WriteRegStr HKCU "Software\Classes\Directory\shell\rza\shell\01add\command" "" '"$INSTDIR\rza-gui.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\Directory\shell\rza\shell\02zip" "" "Compress to .zip"
  WriteRegStr HKCU "Software\Classes\Directory\shell\rza\shell\02zip\command" "" '"$INSTDIR\rza.exe" compress-zip "%1"'
  WriteRegStr HKCU "Software\Classes\Directory\shell\rza\shell\03targz" "" "Compress to .tar.gz"
  WriteRegStr HKCU "Software\Classes\Directory\shell\rza\shell\03targz\command" "" '"$INSTDIR\rza.exe" compress-targz "%1"'
!macroend

!macro RzaUninstallShellMenu
  ; Remove cascading submenu for each archive extension
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.tar\shell\rza"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.gz\shell\rza"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\rza"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\rza"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.xz\shell\rza"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.zst\shell\rza"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.7z\shell\rza"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.rar\shell\rza"

  ; Remove cascading submenu for all files and directories
  DeleteRegKey HKCU "Software\Classes\*\shell\rza"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\rza"
!macroend
