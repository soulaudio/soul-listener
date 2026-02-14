# Hot-Reload Test Instructions

## How to Test Hot-Reload is Working

### Step 1: Start Hot-Reload Mode
```bash
cargo dev
```

You should see:
```
═════════════════════════════════════════════
     SoulAudio DAP - Development Mode
═════════════════════════════════════════════

Starting hot-reload development mode
Watching: firmware, platform, eink-components, eink-system

Building emulator...
Started in X.Xs

Hot-reload active
Save any .rs or .toml file to trigger rebuild
Press Ctrl+C to stop

Emulator running
```

### Step 2: Edit the Example File

Open `crates/firmware/examples/display_emulator.rs` and make a change:

**Line 96** - Change the header text:
```rust
// FROM:
Text::new("Main Menu", Point::new(20, 35), header_style)

// TO:
Text::new("HOT RELOAD WORKS!", Point::new(20, 35), header_style)
```

### Step 3: Save the File

When you save, you should immediately see:

```
Changes detected - rebuilding...

═════════════════════════════════════════════
     SoulAudio DAP - Development Mode
═════════════════════════════════════════════

Building emulator...
Started in X.Xs

Reload complete
```

The window will close and reopen with your changes!

### Step 4: Verify Changes

Look at the emulator window - the header should now say "HOT RELOAD WORKS!" instead of "Main Menu".

## What Gets Watched

Hot-reload monitors these directories for `.rs` and `.toml` file changes:

- ✅ `crates/firmware/src/` - Main firmware code
- ✅ `crates/firmware/examples/` - Example files (like display_emulator.rs)
- ✅ `crates/firmware/Cargo.toml` - Dependencies
- ✅ `crates/platform/` - Platform abstractions
- ✅ `crates/eink/eink-components/src/` - UI components
- ✅ `crates/eink/eink-system/src/` - Layout system

## Common Issues

### "Changes not detected"
- Make sure you're editing files in the watched directories
- Check file extension is `.rs` or `.toml`
- Wait 500ms - there's a debounce to prevent rapid rebuilds

### "Window closes but doesn't reopen"
- Check terminal for build errors
- Fix the errors and save again
- Hot-reload continues running even after build failures

### "Process won't die"
- On Windows, sometimes the process lingers
- Kill manually: `taskkill /F /IM display_emulator.exe`
- Restart `cargo dev`

## Performance

- **Debounce**: 500ms wait after last file change
- **Rebuild time**: ~2-5 seconds (incremental builds)
- **Total cycle**: Edit → Save → See changes in ~3-7 seconds

## Expected Workflow

1. Run `cargo dev` once
2. Edit code
3. Save file
4. Wait ~3 seconds
5. See changes in new window
6. Repeat steps 2-5

**No need to restart `cargo dev` unless you Ctrl+C it!**

---

**Status**: ✅ Working as of 2026-02-14
