#!/bin/bash
TARGET_DIR=target/AppDir
APP_NAME=ktail

USR_DIR="$TARGET_DIR/usr"
BIN_DIR="$USR_DIR/bin"
LIB_DIR="$USR_DIR/lib"
SHARE_DIR="$USR_DIR/share"

rm -rf "$TARGET_DIR"

mkdir "$TARGET_DIR"
mkdir "$USR_DIR"
mkdir "$BIN_DIR"
mkdir "$LIB_DIR"
mkdir "$SHARE_DIR"

cp "target/release/$APP_NAME" "$BIN_DIR/bin"

APP_RUN_SCRIPT="$TARGET_DIR/AppRun"
echo '#!/bin/sh
HERE=$(dirname $(readlink -f "${0}"))
export LD_LIBRARY_PATH="${HERE}"/usr/lib
export XDG_DATA_DIRS="${HERE}"/usr/share:$XDG_DATA_DIRS
"${HERE}"/usr/bin/bin $@
' > "$APP_RUN_SCRIPT"

chmod +x "$APP_RUN_SCRIPT"

echo "
[Desktop Entry]
Name=$APP_NAME
Exec=bin
Icon=icon
Type=Application
Categories=Utility;
X-AppImage-Version=0.1.0
" > "$TARGET_DIR/$APP_NAME.desktop"

touch "$TARGET_DIR/icon.png"

LIBS=("libgtk-4" "libgio-2.0" "libglib-2.0" "libgobject-2.0"
"libpango-1.0" "libgraphene-1.0" "libpangocairo-1.0" "libpangoxft-1.0"
"libgmodule-2.0" "libgthread-2.0"
)

for LIB in "${LIBS[@]}"
do
  cp -r "/opt/gtk/lib/x86_64-linux-gnu/$LIB"* $LIB_DIR
done

cp "/opt/pango/lib/x86_64-linux-gnu/libpangoft2-1.0.so.0" $LIB_DIR
cp "/opt/gtkSourceView/lib/x86_64-linux-gnu/libgtksourceview-5.so.0" $LIB_DIR


SHARE_CP_PATHS=("glib-2.0/schemas/")
for CP_PATH in "${SHARE_CP_PATHS[@]}"
do
  DST_PATH="$SHARE_DIR/$CP_PATH"
  mkdir -p "$DST_PATH"
  cp -r "/opt/gtk/share/$CP_PATH/"* "$DST_PATH"
done

appimagetool "$TARGET_DIR" KTail-x64.AppImage