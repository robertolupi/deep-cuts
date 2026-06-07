# Deep Cuts — multi-agent collaboration helpers (fish).
#
# Setup (once):
#   set -Ux DEEP_CUTS_DIR ~/src/deep-cuts        # <- your clone location
#   echo 'source $DEEP_CUTS_DIR/tools/collab.fish' >> ~/.config/fish/config.fish
#
# Requires `claude` and `agy` on PATH (else set -Ux CLAUDE_BIN / AGY_BIN to their paths).
# EVERY agent turn goes through tools/collab_agent.py, so it is constrained (no skip-permissions;
# Claude has no general Bash; agy runs --sandbox) and killable with `collab-kill`.

function collab-hub --description "Launch the Deep Cuts Collab Hub dashboard"
    $DEEP_CUTS_DIR/tools/.venv/bin/streamlit run $DEEP_CUTS_DIR/tools/collab_hub.py $argv
end

function collab-claude --description "One constrained Claude turn on the active collab session"
    python3 $DEEP_CUTS_DIR/tools/collab_agent.py run claude $argv
end

function collab-agy --description "One sandboxed agy (Gemini) turn on the active collab session"
    python3 $DEEP_CUTS_DIR/tools/collab_agent.py run agy $argv
end

function collab-status --description "List running collab agents"
    python3 $DEEP_CUTS_DIR/tools/collab_agent.py status
end

function collab-kill --description "PANIC BUTTON: kill all running collab agents"
    python3 $DEEP_CUTS_DIR/tools/collab_agent.py kill
end

# Back-compat: route the old catch-up helpers through the safe wrapper (never the raw CLI).
function claude-catchup --description "Claude catch-up turn (via the kill-switch wrapper)"
    python3 $DEEP_CUTS_DIR/tools/collab_agent.py run claude $argv
end

function agy-catchup --description "agy (Gemini) catch-up turn (via the kill-switch wrapper)"
    python3 $DEEP_CUTS_DIR/tools/collab_agent.py run agy $argv
end
