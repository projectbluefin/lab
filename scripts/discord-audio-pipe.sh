#!/bin/bash

# Configuration and state files
STATE_FILE="/tmp/discord-music-pipe-modules"

start_pipe() {
    if [ -f "$STATE_FILE" ]; then
        echo "Pipe is already running or state file exists. Run 'stop' first."
        exit 1
    fi

    echo "Detecting default audio devices..."
    DEFAULT_SINK=$(pactl get-default-sink)
    DEFAULT_SOURCE=$(pactl get-default-source)
    echo "Default Sink: $DEFAULT_SINK"
    echo "Default Source: $DEFAULT_SOURCE"

    echo "Creating virtual sinks..."
    # Create Music_Sink
    MUSIC_SINK_MOD=$(pactl load-module module-null-sink sink_name=Music_Sink sink_properties=device.description="Music_Sink")
    if [ $? -ne 0 ]; then
        echo "Failed to load Music_Sink module."
        exit 1
    fi
    echo "Created Music_Sink (ID: $MUSIC_SINK_MOD)"

    # Create Virtual_Mic_Sink Null Sink
    VIRT_MIC_SINK_MOD=$(pactl load-module module-null-sink sink_name=Virtual_Mic_Sink sink_properties=device.description="Virtual_Mic_Sink")
    if [ $? -ne 0 ]; then
        echo "Failed to load Virtual_Mic_Sink module."
        pactl unload-module $MUSIC_SINK_MOD
        exit 1
    fi
    echo "Created Virtual_Mic_Sink (ID: $VIRT_MIC_SINK_MOD)"

    # Create Virtual_Mic Source (this avoids .monitor in the source name so Discord/Chromium can see it)
    VIRT_MIC_SRC_MOD=$(pactl load-module module-virtual-source source_name=Virtual_Mic master=Virtual_Mic_Sink.monitor source_properties=device.description="Virtual_Mic")
    if [ $? -ne 0 ]; then
        echo "Failed to load module-virtual-source."
        pactl unload-module $MUSIC_SINK_MOD
        pactl unload-module $VIRT_MIC_SINK_MOD
        exit 1
    fi
    echo "Created Virtual_Mic Source (ID: $VIRT_MIC_SRC_MOD)"

    echo "Setting up loopbacks..."
    # 1. Microphone -> Virtual_Mic_Sink
    MIC_LOOP_MOD=$(pactl load-module module-loopback source="$DEFAULT_SOURCE" sink=Virtual_Mic_Sink)
    echo "Looped default mic to Virtual_Mic_Sink (ID: $MIC_LOOP_MOD)"

    # 2. Music_Sink -> Default Physical Output
    MUSIC_OUT_LOOP_MOD=$(pactl load-module module-loopback source="Music_Sink.monitor" sink="$DEFAULT_SINK")
    echo "Looped Music_Sink to physical output (ID: $MUSIC_OUT_LOOP_MOD)"

    # 3. Music_Sink -> Virtual_Mic_Sink
    MUSIC_MIC_LOOP_MOD=$(pactl load-module module-loopback source="Music_Sink.monitor" sink=Virtual_Mic_Sink)
    echo "Looped Music_Sink to Virtual_Mic_Sink (ID: $MUSIC_MIC_LOOP_MOD)"

    # Save loaded module IDs
    echo "$MUSIC_SINK_MOD" > "$STATE_FILE"
    echo "$VIRT_MIC_SINK_MOD" >> "$STATE_FILE"
    echo "$VIRT_MIC_SRC_MOD" >> "$STATE_FILE"
    echo "$MIC_LOOP_MOD" >> "$STATE_FILE"
    echo "$MUSIC_OUT_LOOP_MOD" >> "$STATE_FILE"
    echo "$MUSIC_MIC_LOOP_MOD" >> "$STATE_FILE"

    echo "Locating YouTube Music Desktop app audio stream..."
    MUSIC_APP_ID=$(pactl list sink-inputs | awk '/Sink Input #/ {id=$3} /application.process.binary = "youtube-music-desktop-app"/ {print id}' | sed 's/#//')
    if [ -n "$MUSIC_APP_ID" ]; then
        echo "Found YouTube Music stream (ID: $MUSIC_APP_ID). Moving to Music_Sink..."
        pactl move-sink-input "$MUSIC_APP_ID" Music_Sink
        echo "Successfully moved music stream."
    else
        echo "YouTube Music stream not found or not active. Please start playing music, then run this command again to move the stream, or manually move it using pavucontrol."
    fi

    echo "--- Setup Complete! ---"
    echo "1. Open Discord Settings -> Voice & Video"
    echo "2. Set your Input Device (Microphone) to 'Virtual_Mic'"
    echo "3. Ensure your Output Device remains on your normal headphones/speakers"
}

stop_pipe() {
    if [ ! -f "$STATE_FILE" ]; then
        echo "No active pipe session found. Attempting fuzzy cleanup..."
        # Fallback cleanup by unloading modules matching description
        echo "Cleaning up any loaded loopbacks/null sinks..."
        pactl list modules | grep -E "Module #[0-9]+" -B 1 -A 5 | grep -E "Music_Sink|Virtual_Mic" | grep -o "[0-9]\+" | uniq | while read -r mod_id; do
            if [ -n "$mod_id" ]; then
                echo "Unloading module $mod_id"
                pactl unload-module "$mod_id" 2>/dev/null
            fi
        done
        exit 0
    fi

    echo "Stopping pipe and unloading modules..."
    while read -r mod_id; do
        if [ -n "$mod_id" ]; then
            echo "Unloading module: $mod_id"
            pactl unload-module "$mod_id" 2>/dev/null
        fi
    done < "$STATE_FILE"

    rm -f "$STATE_FILE"
    echo "Piping stopped. Default audio routing restored."
}

case "$1" in
    start)
        start_pipe
        ;;
    stop)
        stop_pipe
        ;;
    *)
        echo "Usage: $0 {start|stop}"
        exit 1
        ;;
esac
