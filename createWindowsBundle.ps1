$appName = "ktail.exe"

$root = "D:\a\_temp\msys64\mingw64"
Get-ChildItem -Recurse $root
$libSrc = "$root\bin"
$libs = "libgtksourceview-5-0.dll", "libbrotlicommon.dll", "libbrotlidec.dll", "libbz2-1.dll", "libcairo-2.dll", "libcairo-gobject-2.dll", "libcairo-script-interpreter-2.dll", "libdatrie-1.dll", "libepoxy-0.dll", "libexpat-1.dll", "libffi-7.dll", "libfontconfig-1.dll", "libfreetype-6.dll", "libfribidi-0.dll", "libgcc_s_seh-1.dll", "libgdk_pixbuf-2.0-0.dll", "libgio-2.0-0.dll", "libglib-2.0-0.dll", "libgmodule-2.0-0.dll", "libgobject-2.0-0.dll", "libgraphene-1.0-0.dll", "libgraphite2.dll", "libgtk-4-1.dll", "libharfbuzz-0.dll", "libiconv-2.dll", "libintl-8.dll", "liblzo2-2.dll", "libpango-1.0-0.dll", "libpangocairo-1.0-0.dll", "libpangoft2-1.0-0.dll", "libpangowin32-1.0-0.dll", "libpcre-1.dll", "libpixman-1-0.dll", "libpng16-16.dll", "libstdc++-6.dll", "libthai-0.dll", "libvulkan-1.dll", "libwinpthread-1.dll", "zlib1.dll"
mkdir bundle

copy "target\release\$appName" .\bundle
foreach($lib in $libs)
{
    copy $libSrc\$lib .\bundle
}

mkdir bundle\lib\gdk-pixbuf-2.0\2.10.0\loaders
copy $root\lib\gdk-pixbuf-2.0\2.10.0\loaders\libpixbufloader-png.dll .\bundle\lib\gdk-pixbuf-2.0\2.10.0\loaders
copy $root\lib\gdk-pixbuf-2.0\2.10.0\loaders.cache .\bundle\lib\gdk-pixbuf-2.0\2.10.0

mkdir bundle\share\glib-2.0\schemas
copy $root\share\glib-2.0\schemas\* .\bundle\share\glib-2.0\schemas\

$iconSrc = "$root\share\icons\Adwaita"
$iconDst = "bundle\share\icons"
$icons = "index.theme", "16x16\actions\document-open-recent-symbolic.symbolic.png", "16x16\actions\edit-clear-symbolic.symbolic.png", "16x16\actions\edit-delete-symbolic.symbolic.png", "16x16\actions\edit-find-symbolic.symbolic.png", "16x16\actions\go-bottom-symbolic.symbolic.png", "16x16\actions\list-add-symbolic.symbolic.png", "16x16\actions\list-remove-symbolic.symbolic.png", "16x16\actions\media-eject-symbolic.symbolic.png", "16x16\actions\object-select-symbolic.symbolic.png", "16x16\actions\open-menu-symbolic.symbolic.png", "16x16\devices\ac-adapter-symbolic.symbolic.png", "16x16\devices\audio-card-symbolic.symbolic.png", "16x16\devices\audio-headphones-symbolic.symbolic.png", "16x16\devices\audio-headphones.png", "16x16\devices\audio-headset-symbolic.symbolic.png", "16x16\devices\audio-headset.png", "16x16\devices\audio-input-microphone-symbolic.symbolic.png", "16x16\devices\audio-speakers-symbolic.symbolic.png", "16x16\devices\auth-fingerprint-symbolic.symbolic.png", "16x16\devices\auth-sim-symbolic.symbolic.png", "16x16\devices\auth-smartcard-symbolic.symbolic.png", "16x16\devices\battery-symbolic.symbolic.png", "16x16\devices\bluetooth-symbolic.symbolic.png", "16x16\devices\camera-photo-symbolic.symbolic.png", "16x16\devices\camera-video-symbolic.symbolic.png", "16x16\devices\camera-web-symbolic.symbolic.png", "16x16\devices\colorimeter-colorhug-symbolic.symbolic.png", "16x16\devices\computer-apple-ipad-symbolic.symbolic.png", "16x16\devices\computer-symbolic.symbolic.png", "16x16\devices\computer.png", "16x16\devices\display-projector-symbolic.symbolic.png", "16x16\devices\drive-harddisk-ieee1394-symbolic.symbolic.png", "16x16\devices\drive-harddisk-solidstate-symbolic.symbolic.png", "16x16\devices\drive-harddisk-symbolic.symbolic.png", "16x16\devices\drive-harddisk-system-symbolic.symbolic.png", "16x16\devices\drive-harddisk-usb-symbolic.symbolic.png", "16x16\devices\drive-harddisk.png", "16x16\devices\drive-multidisk-symbolic.symbolic.png", "16x16\devices\drive-optical-symbolic.symbolic.png", "16x16\devices\drive-removable-media-symbolic.symbolic.png", "16x16\devices\drive-removable-media.png", "16x16\devices\input-dialpad-symbolic.symbolic.png", "16x16\devices\input-gaming-symbolic.symbolic.png", "16x16\devices\input-keyboard-symbolic.symbolic.png", "16x16\devices\input-mouse-symbolic.symbolic.png", "16x16\devices\input-tablet-symbolic.symbolic.png", "16x16\devices\input-touchpad-symbolic.symbolic.png", "16x16\devices\media-flash-symbolic.symbolic.png", "16x16\devices\media-floppy-symbolic.symbolic.png", "16x16\devices\media-optical-bd-symbolic.symbolic.png", "16x16\devices\media-optical-cd-audio-symbolic.symbolic.png", "16x16\devices\media-optical-dvd-symbolic.symbolic.png", "16x16\devices\media-optical-symbolic.symbolic.png", "16x16\devices\media-optical.png", "16x16\devices\media-removable-symbolic.symbolic.png", "16x16\devices\media-tape-symbolic.symbolic.png", "16x16\devices\media-zip-symbolic.symbolic.png", "16x16\devices\modem-symbolic.symbolic.png", "16x16\devices\multimedia-player-apple-ipod-touch-symbolic.symbolic.png", "16x16\devices\multimedia-player-symbolic.symbolic.png", "16x16\devices\network-cellular-symbolic.symbolic.png", "16x16\devices\network-wired-symbolic.symbolic.png", "16x16\devices\network-wireless-symbolic.symbolic.png", "16x16\devices\pda-symbolic.symbolic.png", "16x16\devices\phone-apple-iphone-symbolic.symbolic.png", "16x16\devices\phone-old-symbolic.symbolic.png", "16x16\devices\phone-symbolic.symbolic.png", "16x16\devices\printer-network-symbolic.symbolic.png", "16x16\devices\printer-network.png", "16x16\devices\printer-symbolic.symbolic.png", "16x16\devices\printer.png", "16x16\devices\scanner-symbolic.symbolic.png", "16x16\devices\thunderbolt-symbolic.symbolic.png", "16x16\devices\tv-symbolic.symbolic.png", "16x16\devices\uninterruptible-power-supply-symbolic.symbolic.png", "16x16\devices\video-display-symbolic.symbolic.png", "16x16\devices\video-joined-displays-symbolic.symbolic.png", "16x16\devices\video-single-display-symbolic.symbolic.png", "16x16\mimetypes\application-certificate-symbolic.symbolic.png", "16x16\mimetypes\application-certificate.png", "16x16\mimetypes\application-rss+xml-symbolic.symbolic.png", "16x16\mimetypes\application-x-addon-symbolic.symbolic.png", "16x16\mimetypes\application-x-addon.png", "16x16\mimetypes\application-x-appliance-symbolic.symbolic.png", "16x16\mimetypes\application-x-executable-symbolic.symbolic.png", "16x16\mimetypes\application-x-executable.png", "16x16\mimetypes\application-x-firmware-symbolic.symbolic.png", "16x16\mimetypes\application-x-firmware.png", "16x16\mimetypes\audio-x-generic-symbolic.symbolic.png", "16x16\mimetypes\audio-x-generic.png", "16x16\mimetypes\font-x-generic-symbolic.symbolic.png", "16x16\mimetypes\font-x-generic.png", "16x16\mimetypes\image-x-generic-symbolic.symbolic.png", "16x16\mimetypes\image-x-generic.png", "16x16\mimetypes\inode-directory-symbolic.symbolic.png", "16x16\mimetypes\inode-directory.png", "16x16\mimetypes\package-x-generic-symbolic.symbolic.png", "16x16\mimetypes\package-x-generic.png", "16x16\mimetypes\text-html.png", "16x16\mimetypes\text-x-generic-symbolic.symbolic.png", "16x16\mimetypes\text-x-generic-template.png", "16x16\mimetypes\text-x-generic.png", "16x16\mimetypes\text-x-preview.png", "16x16\mimetypes\text-x-script.png", "16x16\mimetypes\video-x-generic-symbolic.symbolic.png", "16x16\mimetypes\video-x-generic.png", "16x16\mimetypes\x-office-address-book-symbolic.symbolic.png", "16x16\mimetypes\x-office-address-book.png", "16x16\mimetypes\x-office-calendar-symbolic.symbolic.png", "16x16\mimetypes\x-office-calendar.png", "16x16\mimetypes\x-office-document-symbolic.symbolic.png", "16x16\mimetypes\x-office-document-template.png", "16x16\mimetypes\x-office-document.png", "16x16\mimetypes\x-office-drawing-symbolic.symbolic.png", "16x16\mimetypes\x-office-drawing-template.png", "16x16\mimetypes\x-office-drawing.png", "16x16\mimetypes\x-office-presentation-symbolic.symbolic.png", "16x16\mimetypes\x-office-presentation-template.png", "16x16\mimetypes\x-office-presentation.png", "16x16\mimetypes\x-office-spreadsheet-symbolic.symbolic.png", "16x16\mimetypes\x-office-spreadsheet-template.png", "16x16\mimetypes\x-office-spreadsheet.png", "16x16\mimetypes\x-package-repository.png", "16x16\places\folder-documents-symbolic.symbolic.png", "16x16\places\folder-documents.png", "16x16\places\folder-download-symbolic.symbolic.png", "16x16\places\folder-download.png", "16x16\places\folder-drag-accept.png", "16x16\places\folder-music-symbolic.symbolic.png", "16x16\places\folder-music.png", "16x16\places\folder-open.png", "16x16\places\folder-pictures-symbolic.symbolic.png", "16x16\places\folder-pictures.png", "16x16\places\folder-publicshare-symbolic.symbolic.png", "16x16\places\folder-publicshare.png", "16x16\places\folder-remote-symbolic.symbolic.png", "16x16\places\folder-remote.png", "16x16\places\folder-saved-search-symbolic.symbolic.png", "16x16\places\folder-saved-search.png", "16x16\places\folder-symbolic.symbolic.png", "16x16\places\folder-templates-symbolic.symbolic.png", "16x16\places\folder-templates.png", "16x16\places\folder-videos-symbolic.symbolic.png", "16x16\places\folder-videos.png", "16x16\places\folder.png", "16x16\places\network-server-symbolic.symbolic.png", "16x16\places\network-server.png", "16x16\places\network-workgroup-symbolic.symbolic.png", "16x16\places\network-workgroup.png", "16x16\places\start-here-symbolic.symbolic.png", "16x16\places\start-here.png", "16x16\places\user-bookmarks-symbolic.symbolic.png", "16x16\places\user-bookmarks.png", "16x16\places\user-desktop-symbolic.symbolic.png", "16x16\places\user-desktop.png", "16x16\places\user-home-symbolic.symbolic.png", "16x16\places\user-home.png", "16x16\places\user-trash-symbolic.symbolic.png", "16x16\places\user-trash.png", "16x16\status\mail-unread-symbolic.png", "16x16\ui\checkbox-checked-symbolic.symbolic.png", "16x16\ui\checkbox-mixed-symbolic.symbolic.png", "16x16\ui\checkbox-symbolic.symbolic.png", "16x16\ui\focus-legacy-systray-symbolic.symbolic.png", "16x16\ui\focus-top-bar-symbolic.symbolic.png", "16x16\ui\focus-windows-symbolic.symbolic.png", "16x16\ui\list-drag-handle-symbolic.symbolic.png", "16x16\ui\pan-down-symbolic.symbolic.png", "16x16\ui\pan-end-symbolic-rtl.symbolic.png", "16x16\ui\pan-end-symbolic.symbolic.png", "16x16\ui\pan-start-symbolic-rtl.symbolic.png", "16x16\ui\pan-start-symbolic.symbolic.png", "16x16\ui\pan-up-symbolic.symbolic.png", "16x16\ui\radio-checked-symbolic.symbolic.png", "16x16\ui\radio-mixed-symbolic.symbolic.png", "16x16\ui\radio-symbolic.symbolic.png", "16x16\ui\selection-end-symbolic-rtl.symbolic.png", "16x16\ui\selection-end-symbolic.symbolic.png", "16x16\ui\selection-start-symbolic-rtl.symbolic.png", "16x16\ui\selection-start-symbolic.symbolic.png", "16x16\ui\tab-new-symbolic.symbolic.png", "16x16\ui\window-close-symbolic.symbolic.png", "16x16\ui\window-maximize-symbolic.symbolic.png", "16x16\ui\window-minimize-symbolic.symbolic.png", "16x16\ui\window-new-symbolic.symbolic.png", "16x16\ui\window-restore-symbolic.symbolic.png"
foreach($icon in $icons)
{
    xcopy "$iconSrc\$icon" "$iconDst\$icon"*
}
