# rm -r ktail
mkdir ktail\config\

copy target\release\ktail.exe .\ktail
copy config\log4rs.yaml .\ktail\config

pushd ktail
mkdir libs
$root = "D:\a\_temp\msys\msys64\mingw64"

$libSrc = "$root\bin"
$libs = "libatk-1.0-0.dll", "libbrotlicommon.dll", "libbrotlidec.dll", "libbz2-1.dll", "libcairo-2.dll", "libcairo-gobject-2.dll", "libcrypto-1_1-x64.dll", "libdatrie-1.dll", "libepoxy-0.dll", "libexpat-1.dll", "libffi-7.dll", "libfontconfig-1.dll", "libfreetype-6.dll", "libfribidi-0.dll", "libgcc_s_seh-1.dll", "libgdk-3-0.dll", "libgdk_pixbuf-2.0-0.dll", "libgio-2.0-0.dll", "libglib-2.0-0.dll", "libgmodule-2.0-0.dll", "libgobject-2.0-0.dll", "libgraphite2.dll", "libgtk-3-0.dll", "libgtksourceview-3.0-1.dll", "libharfbuzz-0.dll", "libiconv-2.dll", "libintl-8.dll", "liblzma-5.dll", "libpango-1.0-0.dll", "libpangocairo-1.0-0.dll", "libpangoft2-1.0-0.dll", "libpangowin32-1.0-0.dll", "libpcre-1.dll", "libpixman-1-0.dll", "libpng16-16.dll", "libssl-1_1-x64.dll", "libssp-0.dll", "libstdc++-6.dll", "libthai-0.dll", "libwinpthread-1.dll", "libxml2-2.dll", "zlib1.dll"
foreach($lib in $libs)
{
    copy $libSrc\$lib .\
}

mkdir lib\gdk-pixbuf-2.0\2.10.0\loaders
copy $root\lib\gdk-pixbuf-2.0\2.10.0\loaders\libpixbufloader-png.dll .\lib\gdk-pixbuf-2.0\2.10.0\loaders
copy $root\lib\gdk-pixbuf-2.0\2.10.0\loaders.cache .\lib\gdk-pixbuf-2.0\2.10.0

mkdir share\glib-2.0\schemas
copy $root\share\glib-2.0\schemas\* .\share\glib-2.0\schemas\

mkdir share\icons\Adwaita\16x16\
copy $root\share\icons\icon-theme.cache .\share\icons\Adwaita\
copy $root\share\icons\index.theme .\share\icons\Adwaita\index.theme

mkdir share\icons\Adwaita\16x16\actions
copy $root\share\icons\Adwaita\16x16\actions\* .\share\icons\Adwaita\16x16\actions

mkdir share\icons\Adwaita\16x16\devices
copy $root\share\icons\Adwaita\16x16\devices\* .\share\icons\Adwaita\16x16\devices

mkdir share\icons\Adwaita\16x16\places
copy $root\share\icons\Adwaita\16x16\places\* .\share\icons\Adwaita\16x16\places

mkdir share\icons\Adwaita\16x16\ui
copy $root\share\icons\Adwaita\16x16\ui\* .\share\icons\Adwaita\16x16\ui

popd