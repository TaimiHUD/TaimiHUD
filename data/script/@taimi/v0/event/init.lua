local util = require"@taimi/util"
local event = require"@taimi/core/event"

local export = {}

local Event = {
	i = {
		HostSignal = event.HostSignal,
		ScriptSignal = event.ScriptSignal,
	},
	mt = {},
}
function Event.mt:__index(k)
	return Event.i[k]
end
function Event.for_ctx(ctx)
	local i = {
		ctx = ctx,
		plug = ctx.plug,
		events = ctx.events,
	}
	return util.setmetatable(i, Event.mt)
end
function Event.i:Runner(...)
	return self.events:Runner(...)
end

export.Event = {
	for_ctx = Event.for_ctx,
}

return export
