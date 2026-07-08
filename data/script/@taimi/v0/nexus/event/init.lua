local util = require"@taimi/util"
local nexus = require"@taimi/core/nexus"

local export = {
	HostSignal = nexus.HostSignal,
}

local Signal = {
	i = {
		Ids = export.HostSignal,
	},
	mt = {},
}
function Signal.mt:__index(k)
	return Signal.i[k]
end
function Signal.for_ctx(ctx)
	local i = {
		ctx = ctx,
	}
	return util.setmetatable(i, Signal.mt)
end

export.Signal = {
	for_ctx = Signal.for_ctx,
}

local Event = {
	i = {},
	mt = {},
}
function Event.mt:__index(k)
	return Event.i[k]
end
function Event.for_ctx(ctx)
	local i = {
		ctx = ctx,
	}
	return util.setmetatable(i, Event.mt)
end

export.Event = {
	for_ctx = Event.for_ctx,
}

local EventId = {
	i = {},
	mt = {},
}
function EventId.mt:__index(k)
	return EventId.i[k]
end
function EventId.mt:__tostring()
	return self.name or "EV_UNKNOWN"
end
function EventId.new(name)
	local i = {
		name = name,
	}
	return util.setmetatable(i, EventId.mt)
end

export.EventId = {
	new = EventId.new,
}

local Named = {
	-- nexus
	AddonLoaded = "EV_ADDON_LOADED",
	AddonUnloaded = "EV_ADDON_UNLOADED",
	LanguageChanged = "EV_LANGUAGE_CHANGED",
	VolatileAddonDisabled = "EV_VOLATILE_ADDON_DISABLED",
	MumbleIdentityUpdated = "EV_MUMBLE_IDENTITY_UPDATED",
	WindowResized = "EV_WINDOW_RESIZED",
	InputBindUpdated = "EV_INPUTBIND_UPDATED",
	-- rtapi
	RtapiGroupMemberJoined = "RTAPI_GROUP_MEMBER_JOINED",
	RtapiGroupMemberLeft = "RTAPI_GROUP_MEMBER_LEFT",
	RtapiGroupMemberUpdated = "RTAPI_GROUP_MEMBER_UPDATED",
	-- arcdps (bridge)
	AccountName = "EV_ACCOUNT_NAME",
	RequestAccountName = "EV_REQUEST_ACCOUNT_NAME",
	ArcSelfJoin = "EV_ARCDPS_SELF_JOIN",
	ArcSelfLeave = "EV_ARCDPS_SELF_LEAVE",
	ArcSquadJoin = "EV_ARCDPS_SQUAD_JOIN",
	ArcSquadLeave = "EV_ARCDPS_SQUAD_LEAVE",
	RealtimeTargetChanged = "EV_ARCDPS_TARGET_CHANGED",
	RealtimeCombatLocal = "EV_ARCDPS_COMBATEVENT_LOCAL_RAW",
	RealtimeCombatSquad = "EV_ARCDPS_COMBATEVENT_SQUAD_RAW",
	ReplaySquadJoin = "EV_REPLAY_ARCDPS_SQUAD_JOIN",
	ReplaySelfJoin = "EV_REPLAY_ARCDPS_SELF_JOIN",
	ReplayTargetChanged = "EV_REPLAY_ARCDPS_TARGET_CHANGED",
	-- unofficial_extras
	UeSquadUpdate = "EV_UNOFFICIAL_EXTRAS_SQUAD_UPDATE",
	UeLanguageChanged = "EV_UNOFFICIAL_EXTRAS_LANGUAGE_CHANGED",
	UeKeybindChanged = "EV_UNOFFICIAL_EXTRAS_KEYBIND_CHANGED",
	UeChatMessage = "EV_UNOFFICIAL_EXTRAS_CHAT_MESSAGE",
}
export.NexusEvents = util.table_map_collect(EventId.new, Named)
Signal.i.Ids = export.NexusEvents

return export
