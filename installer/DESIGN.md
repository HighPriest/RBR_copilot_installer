# Installer UI design file

The installer is made to provide streamlined download and installation process of mod packages for multiple games.  
For now, these games are going to be "Richard Burns Rally", "Dirt Rally" & "BeamNG".  
Because the user is most likely going to desire modification of only one of these applications, the target game should be selectable as a tab on a horizontal ribbon on top of the application.  
All tabs should have identical layout and content, if not stated otherwise in the design.  

## Language selection

When the application is started, a small language selection widget should be shown.  
To change language, the application has to be restarted.

## Mod management UI

Inside each tab, on top, a directory input should be present, to enter directory of the chosen game. This field should have a button which opens directory selector & the field should be validated.  
Richard Burns Rally is going to need "richardburnsrally_sse.exe", Dirt rally requires "rally.exe" & BeamNG requires "BeamNG.drive.exe"  
Beneath, a two column layout should exist.
Left side, should hold lists of already downloaded content, to enable & install. Separate lists for different category of modification (voice, ui language, callouts style). In each category, single item has to be selectable. An install button should be present, which checks if item from each category has been pressed and then proceeds with installation.

Right side, should have tabs "Download", "Backups".
The Download section (default section) holds lists of content available for download. Separate lists for different category of modification (voice, ui language, callouts style)  
Download section should include space to enter private content key. If entered, lists should be refreshed.  

Beneath the two column layout, a status bar should exist, which tracks current download or installation status.
The status bar should include text on which file it is currently working on & file size information during download process.

At the bottom, a text area should be present, for notes and additional information for the user and a separate tab for showing logs. If an error happens, then the window should automatically switch to the logs tab, to inform the user about the error.
