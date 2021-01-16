#!/bin/sh

mv ./target/release/bundle/osx/ktail.app/Contents/MacOS/ktail ./target/release/bundle/osx/ktail.app/Contents/MacOS/ktail-bin
chmod +x target/release/bundle/osx/ktail.app/Contents/MacOS/ktail-bin

cp ./assets/MacOS/ktail ./target/release/bundle/osx/ktail.app/Contents/MacOS/
chmod +x target/release/bundle/osx/ktail.app/Contents/MacOS/ktail

cp ./assets/MacOS/gdk-pixbuf-query-loaders ./target/release/bundle/osx/ktail.app/Contents/MacOS/
chmod +x ./target/release/bundle/osx/ktail.app/Contents/MacOS/gdk-pixbuf-query-loaders

cp -R ./assets/MacOS/Resources ./target/release/bundle/osx/ktail.app/Contents/
cp -R ./assets/MacOS/lib ./target/release/bundle/osx/ktail.app/Contents/MacOS/