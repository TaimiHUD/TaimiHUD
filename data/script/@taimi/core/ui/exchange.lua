-- stub for testing; core modules are built-in to scripting engine
local uix = {}

function uix.info_start(msg)
	require("@taimi/core/log").Info(string.format("UI INFO: %s", msg))
end
function uix.info_end(token)
	require("@taimi/core/log").Debug("UI INFO END")
end
function uix.info_notify(msg, dur)
	require("@taimi/core/log").Info(string.format("UI NOTIF: %s for %fs", msg, dur))
end
function uix.clipboard_send(value)
	require("@taimi/core/log").Debug(string.format("UI COPY: %s", value))
end

return uix
