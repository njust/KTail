# Kubernetes log viewer

KTail allows you to tail multiple pods in one view. It automatically detects updates and attaches to new pods. Configurable highlighters show how often regular expressions matched and let you quickly navigate in the results.

# Build
For all platforms: https://www.rust-lang.org/tools/install

## Windows prerequisites
You need to have msys2 with the following packages installed

- mingw-w64-x86_64-toolchain
- base-devel
- mingw-w64-x86_64-gtk3
- mingw-w64-x86_64-gtksourceview3

## Linux (debian based) prerequisites 
- libgtksourceviewmm-3.0-dev

## MacOS
- gtk+3 
- gtksourceview3