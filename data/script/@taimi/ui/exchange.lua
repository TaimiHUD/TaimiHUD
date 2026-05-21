local uix = require"@taimi/core/ui/exchange"

local Clipboard = {}
function Clipboard.Send(value)
	uix.clipboard_send(value)
end

local Alert = {
	i = {}
}
Alert.mt = { __index = Alert.i }
function Alert:New(message, i)
	i = util.setmetatable(i or {}, self.mt)
	i.message = message
	return i
end
function Alert.new(...)
	return Alert:New(...)
end

function Alert.i:Show(timeout)
	-- error("stub: Alert:Show")
	if timeout == 0 then
		uix.info_notify(self.message)
	else
		self.token = uix.info_start(self.message, timeout)
	end
end
function Alert.i:Hide()
	if self.token == nil then return end
	uix.info_end(self.token)
	self.token = nil
end

function Alert.NotifyClipboard(value, message)
	if message ~= nil then
		uix.info_notify(string.format("%s\n`%s`", message, value))
	else
		uix.info_notify(string.format("copied to clipboard: `%s`", value))
	end
end

return {
	Clipboard = Clipboard,
	Alert = Alert,
}
