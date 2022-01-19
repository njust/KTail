#!/bin/sh

APP_NAME="KTail.app"
BIN_NAME="ktail"
BUNDLE_DIR="./target/release/bundle/osx/$APP_NAME"
BUNDLE_CONTENT_DIR="$BUNDLE_DIR/Contents"
BUNDLE_MACOS_DIR="$BUNDLE_CONTENT_DIR/MacOS"
BUNDLE_RES_DIR="$BUNDLE_CONTENT_DIR/Resources"

mv "$BUNDLE_MACOS_DIR/$BIN_NAME" "$BUNDLE_MACOS_DIR/$BIN_NAME"-bin
chmod +x "$BUNDLE_MACOS_DIR/$BIN_NAME"-bin

echo '#!/bin/sh
MAC_OS_DIR=$(cd "$(dirname "$0")"; pwd)
ROOT=`dirname "$MAC_OS_DIR"`
LIB_DIR="$MAC_OS_DIR"/lib
RESOURCE_DIR="$ROOT"/Resources

export LD_LIBRARY_PATH="$LIB_DIR"
export DYLD_LIBRARY_PATH="$LIB_DIR"
export GTK_PATH="$LIB_DIR"
export GTK_DATA_PREFIX="$RESOURCE_DIR"
export XDG_DATA_DIRS="$RESOURCE_DIR"
export GDK_PIXBUF_MODULE_FILE="$LIB_DIR/gdk-pixbuf-2.0/loaders.cache"
export GDK_PIXBUF_MODULEDIR="$LIB_DIR/gdk-pixbuf-2.0/loaders"
export PANGO_LIBDIR="$LIB_DIR"

$EXEC "$MAC_OS_DIR/ktail-bin"
' > "$BUNDLE_MACOS_DIR/$BIN_NAME"
chmod +x "$BUNDLE_MACOS_DIR/$BIN_NAME"

LIB_SRC="/usr/local//lib"
LIB_DIR="$BUNDLE_DIR/Contents/MacOS/lib"
mkdir "$LIB_DIR"

cp "$LIB_SRC/libgtk-4.1.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgio-2.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libglib-2.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgtksourceview-5.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgobject-2.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libpango-1.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgraphene-1.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libpangoft2-1.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libpangocairo-1.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgmodule-2.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgthread-2.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libbrotlicommon.dylib" "$LIB_DIR"
cp "$LIB_SRC/libcairo.2.dylib" "$LIB_DIR"
cp "$LIB_SRC/libcairo-gobject.2.dylib" "$LIB_DIR"
cp "$LIB_SRC/libepoxy.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libfontconfig.1.dylib" "$LIB_DIR"
cp "$LIB_SRC/libfreetype.6.dylib" "$LIB_DIR"
cp "$LIB_SRC/libfribidi.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgdk_pixbuf-2.0.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgraphite2.3.dylib" "$LIB_DIR"
cp "$LIB_SRC/libgraphite2.dylib" "$LIB_DIR"
cp "$LIB_SRC/libharfbuzz.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libintl.8.dylib" "$LIB_DIR"
cp "$LIB_SRC/libpcre2-8.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libpcre.1.dylib" "$LIB_DIR"
cp "$LIB_SRC/libpixman-1.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libcairo-script-interpreter.2.dylib" "$LIB_DIR"
cp "/usr/lib/libffi.dylib" "$LIB_DIR/libffi.8.dylib"
cp "$LIB_SRC/libpng16.16.dylib" "$LIB_DIR"
cp "$LIB_SRC/libxcb-shm.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libxcb.1.dylib" "$LIB_DIR"
cp "$LIB_SRC/libxcb-render.0.dylib" "$LIB_DIR"
cp "$LIB_SRC/libXrender.1.dylib" "$LIB_DIR"
cp "$LIB_SRC/libX11.6.dylib" "$LIB_DIR"
cp "$LIB_SRC/libXext.6.dylib" "$LIB_DIR"
cp "$LIB_SRC/libXau.6.dylib" "$LIB_DIR"
cp "$LIB_SRC/libXdmcp.6.dylib" "$LIB_DIR"
cp "$LIB_SRC/libjpeg.9.dylib" "$LIB_DIR"
cp "$LIB_SRC/liblzo2.2.dylib" "$LIB_DIR"
cp "/usr/local/Cellar/openssl@1.1/1.1.1m/lib/libssl.1.1.dylib" "$LIB_DIR"

mkdir "$LIB_DIR/gdk-pixbuf-2.0"
cp -R -L /usr/local/lib/gdk-pixbuf-2.0/2.10.0/ "$LIB_DIR/gdk-pixbuf-2.0"

# Copy glib schemas (for file chooser dlg, etc)
cp -R -L /usr/local/share/glib-2.0 "$BUNDLE_RES_DIR"

mkdir "$BUNDLE_RES_DIR/icons"
cp -R -L /usr/local/share/icons/Adwaita "$BUNDLE_RES_DIR/icons"

cd ./target/release/bundle/osx/
hdiutil create "$BIN_NAME".dmg -volname "$BIN_NAME Installer" -fs HFS+ -srcfolder $APP_NAME