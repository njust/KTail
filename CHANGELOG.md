# Version 0.3.4
- Added button in pod selection dialog to refresh the list of pods

# Version 0.3.3
- Log view now automatically attaches to all containers in a pod
- Monospace font for better readability
- Fixed pod selection sometimes not properly recognize replicas

# Version 0.3.2
- Better contrast for highlighters (contributed by otengler)
- New button to add a separator line (contributed by otengler)
- Added column with last matching line to the result view

# Version 0.3.1
- Added option to specify path to kubernetes config file with the following priority:
    - Command parameter ./ktail -c /path/to/.kube/config
    - KUBECONFIG environment variable
    - Default location at ~/home/.kube/config
- Renamed menu entry from "Kube" to "Kubernetes"
- Fixed icon for bookmarks

# Version 0.3.0
- Result overview for rules
- Quickly find recurring errors and warnings with grouped results
- Improved search performance
- Fixed scroll to next / previous match not working when data was loading at the same time
- Removed minimap overlay

# Version 0.2.6
- Added Linux AppImage 
- Improved macOS menu

# Version 0.2.5
- Added cluster selection
- Added namespace selection
- Added Linux package
- Added MacOS package

# Version 0.2.4
- New default theme
- Added rules to pod selection

# Version 0.2.3
- Added option to show logs since n minutes
- Fixed platform specific key mapping for bookmarks

# Version 0.2.2
- Toggle bookmark with Ctrl + number and jump to bookmark with Alt + number
- Fixed wrong file offset when reloading files from disk
- Fixed wrong offsets in search matches
- Fixed opening files via unc path not working via drag drop

# Version 0.2.1
- Added button to clear log view
- Added option to filter log data
- Added missing icon for color selector
- Fixed newly created tab not selected by default
- Fixed crash when invalid regex was entered for highlighters
- Fixed pod name not displaying correctly for stateful sets
- Fixed pod list always loading on start

# Version 0.2.0
- Close the active tab with Ctrl + W
- 'Open with' works now so you can set kTail as your default program for log files
- Toggling the 'Include replicas' checkbox in the pod selection shows now the expected result
- When multiples replicas are logging into one tab, each line will now be prefixed with the last segment of the pod id

# Version 0.1.3
- Drag & Drop files to open them in log viewer
- Switch tabs with Ctrl + Tab and Ctrl + Shift + Tab
- Reduced bundle size
- Fixed crash

# Version 0.1.2
- Added icon
- Search highlights full line
- Search is now case insensitive
- Show count of highlighter matches

# Version 0.1.1
- Open multiple logs in tabs
- Configure multiple highlighters based on regular expressions
- Navigate in results of highlighters
- Minimap for logs
- Auto refresh & auto scroll
