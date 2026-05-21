local event = require"@taimi/core/event"
local util = require"@taimi/util"

local Event = {
	Unmask = util.fun_bind_method0("Unmask", event.Event),
	Mask = util.fun_bind_method0("Mask", event.Event),
	SignalOob = util.fun_bind_method0("SignalOob", event.Event),
}
--[[function Event.EventLoop(signal)
	local signal = signal or event.ScriptSignal.Started
	return signal
end]]

local EventLoop = {
	i = {
		signal = event.ScriptSignal.Started,
	},
}
local ended = event.ScriptSignal.Ended
local pending = event.ScriptSignal.Pending
EventLoop.mt = { __index = EventLoop.i }
function EventLoop:New(i)
	i = util.setmetatable(i or {}, self.mt)
	if i.queue == nil then
		i.queue = {}
	end
	if i.marker_handlers == nil then
		i.marker_handlers = {}
	end
	if i.handlers == nil then
		i.handlers = {}
	end
	return i
end
function EventLoop.new(...)
	return EventLoop:New(...)
end
function EventLoop.i:Start()
	self.co = coroutine.create(function(...) return self:Run(...) end)
	return self.co
end
function EventLoop.i:Reset()
	self.state = nil
end
function EventLoop.i:IsActive()
	return self.state ~= ended
end
function EventLoop.i:Runner()
	return util.fun_bind1(self.Turn, self)
end
function EventLoop.i:Turn(incoming)
	local active = self:IsActive()
	self:ProcessIncoming(incoming)
	local msg = self:PopOutgoing()
	-- TODO? if msg == ended and not self:IsActive() then msg = nil end
	return msg
end
function EventLoop.i:PopOutgoing()
	local signal = self.signal
	local upcoming = util.array_pop_end(self.queue)
	if not signal and upcoming then
		signal = upcoming
	else
		self.signal = upcoming or false
	end
	return signal or self.state or pending
end
function EventLoop.i:ProcessIncoming(ev)
	local handler = self[ev.id] or self.Unhandled
	handler(self, ev)
end


function EventLoop.i:QueueMessage(ev, signal)
	util.array_push_start(self.queue, ev)
	if signal and self.co ~= nil then
		-- NOTE: this could coroutine.yield if running from inside turn, check a guard somewhere?
		Event.SignalOob(self.co, event.ScriptSignal.Resume)
	end
end

function EventLoop.i:RegisterFunc(id, f)
	if EventLoop.i[id] == nil then
		EventLoop.i[id] = EventLoop.i.HandleGeneric
	end
	local handlers = self.handlers[id]
	if handlers == nil then
		handlers = {}
		self.handlers[id] = handlers
		Event.Unmask(id)
	end
	local idx = util.array_push_end(handlers, f)
	return idx
end
function EventLoop.i:RegisterMarkerFunc(id, markerid, f)
	local handlers = self.marker_handlers[id]
	if handlers == nil then
		if f == nil then
			return
		end
		handlers = {}
		self.marker_handlers[id] = handlers
		Event.Unmask(id)
	end
	handlers[markerid or false] = f
end
-- TODO: Deregister

-- pathing compat wrappers
function EventLoop.i:RegisterAttr(id, f, extra_args)
	local f = self:WrapAttrFunc(f, extra_args)
	return self:RegisterFunc(id, f)
end
function EventLoop.i:RegisterMarkerAttr(id, markerid, f, extra_args)
	local f = self:WrapAttrFunc(f, extra_args)
	return self:RegisterMarkerFunc(id, markerid, f)
end
-- TODO: stash some context in a global or instance field somewhere?
function EventLoop.i:WrapAttrFunc(f, extra_args)
	local lazy_args = util.is_callable(extra_args)
	return function(_eloop, ev, ...)
		local args = {...}
		local extra_args = extra_args
		if lazy_args then
			extra_args = {extra_args()}
		end
		util.array_append(extra_args, 1, args)
		f(util.table.unpack(args))
	end
end

function EventLoop.i:Unhandled(ev)
end
function EventLoop.i:UnhandledMarker(ev, args)
end
function EventLoop.i:HandleGeneric(ev)
	local handler = self.handlers[ev.id]
	if handler == nil then
		handler = {}
		self.handlers[ev.id] = handler
	end
	if handler ~= nil then
		for _,h in ipairs(handler) do
			if h(self, ev, ev:GetArgsPositional()) == false then
				return false
			end
		end
	else
		return self:Unhandled(ev)
	end
end
EventLoop.i[event.HostSignal.Exit] = function(self, ev)
	self.state = ended
end
EventLoop.i[event.HostSignal.PathingTick] = EventLoop.i.HandleGeneric
EventLoop.i[event.HostSignal.PathingMapExit] = function(self, ev)
	local stopped = self:HandleGeneric(ev)
	if stopped == false then return false end
	local _map_id = ev:GetArgsPositional()
	-- clean up marker handlers in preparation for incoming...
	-- TODO: maybe just avoid doing this...
	local function clear_marker_handlers(s)
		if s ~= nil then
			self.marker_handlers[s] = nil
		end
	end
	clear_marker_handlers(event.HostSignal.PathingTickMarker)
	clear_marker_handlers(event.HostSignal.PathingLoadMarker)
	clear_marker_handlers(event.HostSignal.PathingFilterMarker)
	clear_marker_handlers(event.HostSignal.PathingTrigger)
	clear_marker_handlers(event.HostSignal.PathingFocus)
	clear_marker_handlers(event.HostSignal.PathingUnfocus)
end

local poi_tag, trail_tag, poi_wrap, trail_wrap
local function marker_wrap(target)
	if poi_tag == nil then
		local Poi = require("@taimi/compat/poi").Poi
		local Trail = require("@taimi/compat/trail").Trail
		poi_tag = Poi.tag_type
		poi_wrap = Poi.wrap
		trail_tag = Trail.tag_type
		trail_wrap = Trail.wrap
	end
	local targetty = target.PathableTagType
	-- target = target.PathableTagIndex
	if targetty == poi_tag then
		return poi_wrap(target)
	elseif targetty == trail_tag then
		return trail_wrap(target)
	else
		error("unrecognized marker type tag")
	end
end
function EventLoop.i:HandleMarker(ev)
	local args = {ev:GetArgsPositional()}
	local target = false
	if args[1] ~= nil then
		target = args[1]
		if type(args[1]) == "number" then
			args[1] = self.plug:GetWorldHandle():PathableByTag(args[1])
		end
		args[1] = marker_wrap(args[1])
	end
	local handler = self.marker_handlers[ev.id]

	--[[if handler == nil then
		handler = {}
		self.marker_handlers[ev.id] = handler
	end]]
	if handler ~= nil then
		handler = handler[target]
	end

	handler = handler or self.UnhandledMarker
	return handler(self, ev, util.table.unpack(args))
end
function EventLoop.i:HandleMarkerId(ev)
	local args = {ev:GetArgsPositional()}
	local target = false
	if args[1] ~= nil then
		target = args[1]
	end
	local handler = self.marker_handlers[ev.id]

	--[[if handler == nil then
		handler = {}
		self.marker_handlers[ev.id] = handler
	end]]
	if handler ~= nil then
		handler = handler[target]
	end

	handler = handler or self.UnhandledMarker
	return handler(self, ev, util.table.unpack(args))
end
local handle_marker = util.fun_redir_method("HandleMarker")
local function register_marker_handler(id)
	if id ~= nil then
		EventLoop.i[id] = handle_marker
	end
end
local function register_marker_id_handler(id)
	if id ~= nil then
		EventLoop.i[id] = EventLoop.i.HandleMarkerId
	end
end
register_marker_handler(event.HostSignal.PathingTickMarker)
register_marker_handler(event.HostSignal.PathingLoadMarker)
register_marker_handler(event.HostSignal.PathingFilterMarker)
register_marker_handler(event.HostSignal.PathingTrigger)
register_marker_handler(event.HostSignal.PathingFocus)
register_marker_handler(event.HostSignal.PathingUnfocus)
register_marker_id_handler(event.HostSignal.MenuClick)

-- TODO: let controller decide when and which markers to tick...
function EventLoop.i:PrepareMarkerHandlers(pack_info)
	local function tick_markers(self, ev, gametime)
		local handlers = self.marker_handlers[event.HostSignal.PathingTickMarker]
		if handlers == nil then return end

		-- local gametime = ev:GetArgsPositional()
		local lookup = pack_info:GetWorldHandle()
		for markerid, f in pairs(handlers) do
			local marker = lookup:PathableByTag(markerid)
			if marker ~= nil then
				f(self, ev, marker_wrap(marker), gametime)
			end
		end
	end
	self:RegisterFunc(event.HostSignal.PathingTick, tick_markers)

	local binds
	local pathing_hack_interact = require("@taimi/core/rt").pathing_hack_interact
	local function game_bind_compat(self, ev, binds_changed, binds_state)
		local handlers = self.marker_handlers[event.HostSignal.PathingTrigger]
		if handlers == nil then return end

		if binds == nil then
			binds = require"@taimi/bindings"
		end

		-- local binds_changed, binds_state = ev:GetArgsPositional()
		if not binds_changed:GetAt0(binds.ControlNames.Miscellaneous_Interact) or not binds_state:GetAt0(binds.ControlNames.Miscellaneous_Interact) then
			-- we only care about interact press events, ignore release or already held
			return
		end
		local lookup = pack_info:GetWorldHandle()
		local space = pack_info:GetSpaceHandle()
		for markerid, f in pairs(handlers) do
			local marker = lookup:PathableByTag(markerid)
			if marker ~= nil and not marker:GetAttrByKey("autotrigger") then
				local range = marker:GetAttrByKey("triggerrange") or 2.0
				if space:GetDistanceToPlayer(marker) <= range then
					local simulate_interact = pathing_hack_interact and (marker:GetAttrByKey("info") or marker:GetAttrByKey("copy"))
					marker = marker_wrap(marker)
					if simulate_interact then
						marker:Interact(false)
					end
					f(self, ev, marker, false)
				end
			end
		end
	end
	if require("@taimi/core/rt").pathing_hack_manualtrigger then
		self:RegisterFunc(event.HostSignal.GameplayKeybind, game_bind_compat)
	end
end


return util.alias_index_to(event, {
	Event = Event,
	EventLoop = EventLoop,
})
