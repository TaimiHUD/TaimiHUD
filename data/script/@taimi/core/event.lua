-- stub for testing; core modules are built-in to scripting engine
local util = require"@taimi/util"

local Event = {
}
function Event:Unmask(id)
	error("stub: Event:Unmask", 2)
end
function Event:Mask(id)
	return nil
end
function Event:SignalOob(co, id)
	error("stub: Event:SignalOob", 2)
end

local NotifyMessage = {
	i = {},
}
NotifyMessage.mt = { __index = NotifyMessage.i }
function NotifyMessage:New(id, i)
	i = util.setmetatable(i or {}, self.mt)
	i.id = id
	return i
end
function NotifyMessage.new(...)
	return NotifyMessage:New(...)
end
function NotifyMessage.i:GetArgsPositional()
	if self.args == nil then return {} end
	return util.table_copy_shallow(self.args)
end

local EventReceiver = {
	i = {},
}
EventReceiver.mt = { __index = EventReceiver.i }
function EventReceiver:New(name, ...)
	i = util.setmetatable(i or {}, self.mt)
	i.receiver = name
	i.extraArgs = util.table.pack(...)
	return i
end
function EventReceiver.new(...)
	return EventReceiver:New(...)
end
function EventReceiver:CallWith(_g, ...)
	local args = util.table.pack(...)
	array_move(self.extraArgs, 1, self.extraArgs.n, args.n + 1, args)
	_g[self.receiver](util.table.unpack(args))
end

-- numbers not real btw
local HostSignal = {
	Exit = 1,
	Nop = 2,
	PathingTick = 3,
	PathingTickMarker = 4,
	PathingLoadMarker = 5,
	PathingFilterMarker = 6,
	PathingTrigger = 7,
	PathingFocus = 8,
	PathingUnfocus = 9,
	PathingMapExit = 10,
	DebugWatchExport = 11,
	MenuClick = 12,
	GameplayKeybind = 13,
}
local ScriptSignal = {
	Started = 100,
	Pending = 101,
	Ended = 102,
	Resume = 103,
	Restart = 104,
	PathingHideMarker = 105,
	PathingShowMarker = 106,
}

return {
	NotifyMessage = NotifyMessage,
	EventReceiver = EventReceiver,
	HostSignal = HostSignal,
	ScriptSignal = ScriptSignal,
	Event = Event,
}
