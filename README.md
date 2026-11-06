# welffi animate

A Roblox R6 animator for Windows. Pose the rig, key it on a timeline, then copy the animation straight into Studio.

## Get it

Download `welffi animate.exe` from the latest release. It's one file, with nothing to install.

If you use welffi hub, it finds and updates this for you.

## Using it

- Select a part and press R to rotate it or G to move it.
- Press I to key the pose at the current frame.
- Drag keys on the timeline to retime them. Ctrl+Tab switches to the graph editor.
- Press B for build-up and overshoot on selected keys.
- Press Ctrl+Shift+C to copy the animation, then paste it into the Roblox animation importer plugin.

The shortcut list is also shown in the corner of the viewport.

## Building

You need Rust. Run `build.bat`, which makes `dist\welffi animate.exe`. Run `cargo test` for the checks.
