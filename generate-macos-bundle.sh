#!/bin/sh

mv ./target/release/bundle/osx/ktail.app/Contents/MacOS/ktail ./target/release/bundle/osx/ktail.app/Contents/MacOS/ktail-bin
cp ./assets/MacOS/ktail ./target/release/bundle/osx/ktail.app/Contents/MacOS/
cp ./assets/MacOS/gdk-pixbuf-query-loaders ./target/release/bundle/osx/ktail.app/Contents/MacOS/

cp ./assets/MacOS/Resources ./target/release/bundle/osx/ktail.app/Contents/ -R
cp ./assets/MacOS/lib ./target/release/bundle/osx/ktail.app/Contents/MacOS/ -R