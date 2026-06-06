#!/usr/bin/env bash
# <xbar.title>Claude Usage</xbar.title>
# <xbar.version>1.0</xbar.version>
# <xbar.author>Arthur van Rooijen</xbar.author>
# <xbar.desc>Shows Claude.ai usage limits (5-hour and 7-day) in the menu bar.</xbar.desc>
# <xbar.refreshAfterEveryOpenClose>true</xbar.refreshAfterEveryOpenClose>

# Try common install locations for the binary
BINARY=""
for candidate in \
    "${HOME}/.cargo/bin/claude-notify" \
    "/usr/local/bin/claude-notify" \
    "/opt/homebrew/bin/claude-notify" \
    "$(command -v claude-notify 2>/dev/null)"
do
    if [ -x "$candidate" ]; then
        BINARY="$candidate"
        break
    fi
done

if [ -z "$BINARY" ]; then
    echo "Claude ?"
    echo "---"
    echo "claude-notify not found"
    echo "Install it or update the BINARY path in this script."
    exit 0
fi

# Fetch usage output without ANSI colors
OUTPUT=$(NO_COLOR=1 "$BINARY" usage 2>&1)
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    echo "Claude ⚠"
    echo "---"
    echo "Error fetching usage (exit $EXIT_CODE)"
    echo "$OUTPUT" | head -5
    exit 0
fi

# Extract the usage percentages from the 💬 (token) lines
FIVE_HOUR_PCT=$(echo "$OUTPUT" | grep "💬" | sed -n '1p' | grep -oE '[0-9]+\.[0-9]+%')
SEVEN_DAY_PCT=$(echo "$OUTPUT" | grep "💬" | sed -n '2p' | grep -oE '[0-9]+\.[0-9]+%')

# Pick a status indicator based on highest percentage
MAX_PCT=0
for pct in "$FIVE_HOUR_PCT" "$SEVEN_DAY_PCT"; do
    val="${pct/\%/}"
    if [ -n "$val" ] && (( $(echo "$val > $MAX_PCT" | bc -l) )); then
        MAX_PCT="$val"
    fi
done

if (( $(echo "$MAX_PCT >= 90" | bc -l) )); then
    INDICATOR="🔴"
elif (( $(echo "$MAX_PCT >= 50" | bc -l) )); then
    INDICATOR="⚠️"
else
    INDICATOR="✓"
fi

# Menu bar title: compact summary
echo "Claude ${FIVE_HOUR_PCT:-?} / ${SEVEN_DAY_PCT:-?} ${INDICATOR}"
echo "---"

# Dropdown: full usage output, one line per menu item
while IFS= read -r line; do
    # Skip the blank first line
    [ -z "$line" ] && continue
    echo "$line"
done <<< "$OUTPUT"

echo "---"
echo "Refresh | refresh=true"
