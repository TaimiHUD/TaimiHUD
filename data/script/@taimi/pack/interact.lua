local int = require"@taimi/todo/interact"
local bitop = require"@taimi/bitop"
local util = require"@taimi/util"

local CompatNames = {
	-- bhud pathing
	BounceModifier = {
		mask = int.TriggerMask.Bounce,
		kind = int.TriggerKind.Bounce,
		presence = "bounce",
		attrnames = {
			Behavior = "bounce",
			BounceHeight = "bounce-height",
			BounceDuration = "bounce-duration",
			BounceDelay = "bounce-delay",
		},
	},
	CopyModifier = {
		mask = int.TriggerMask.Copy,
		kind = int.TriggerKind.Copy,
		presence = "copy",
		attrnames = {
			CopyValue = "copy",
			CopyMessage = "copy-message",
		},
	},
	InfoModifier = {
		mask = int.TriggerMask.Info,
		kind = int.TriggerKind.Info,
		presence = "info",
		attrnames = {
			InfoValue = "info",
			InfoRange = "inforange",
		},
	},
	ResetGuidModifier = {
		mask = int.TriggerMask.Reset,
		kind = int.TriggerKind.Reset,
		attrnames = {
			TargetGuids = "resetguid",
		},
	},
	ToggleModifier = {
		mask = int.TriggerMask.Toggle,
		kind = int.TriggerKind.Toggle,
		attrnames = {
			Category = "toggle",
		},
	},
	ShowModifier = {
		mask = int.TriggerMask.Show,
		kind = int.TriggerKind.Show,
		attrnames = {
			Category = "show",
		},
	},
	HideModifier = {
		mask = int.TriggerMask.Hide,
		kind = int.TriggerKind.Hide,
		attrnames = {
			Category = "hide",
		},
	},
	StandardBehaviorFilter = {
		-- this one is both a filter and a behaviour (has interact/focus methods + IsFiltered)
		mask = int.TriggerMask.Behaviour,
		kind = int.TriggerMask.Behaviour,
		filter_mask = int.FilterMask.Behaviour,
		attrnames = {
			-- TODO: not actually exposed by blish pathing?
			Behavior = "raid",
		},
	},
	Script = {
		-- also both a behaviour and filter, but separation is messy...
		mask = int.TriggerMask.Script,
		kind = int.TriggerKind.Script,
		-- TODO: presence = { "script-trigger", "script-focus", "script-filter" }
		presence = true,
		attrnames = {
			TickFunc = "script-tick",
			FocusFunc = "script-focus",
			TriggerFunc = "script-trigger",
			FilterFunc = "script-filter",
			OnceFunc = "script-once",
		},
	},
	ScriptFilter = {
		-- TODO: remove/combine or what..?
		mask = int.FilterMask.Script,
		kind = int.FilterKind.Script,
		presence = "script-filter",
		attrnames = {
			TickFunc = "script-tick",
			FocusFunc = "script-focus",
			TriggerFunc = "script-trigger",
			FilterFunc = "script-filter",
			OnceFunc = "script-once",
		},
	},
	AchievementFilter = {
		-- this one is both a filter and a behaviour (has interact/focus methods)
		mask = int.TriggerMask.Behaviour,
		filter_mask = int.FilterMask.Achievement,
		kind = int.FilterKind.Achievement,
		presence = "achievementid",
		attrnames = {
			AchievementId = "achievementid",
			AchievementBit = "achievementbit",
		},
	},
	MountFilter = {
		mask = int.FilterMask.Mount,
		kind = int.FilterKind.Mount,
		attrnames = {
			AllowedMounts = "mount",
		},
	},
	MapTypeFilter = {
		mask = int.FilterMask.MapType,
		kind = int.FilterKind.MapType,
		attrnames = {
			AllowedMapTypes = "maptype",
		},
	},
	RaceFilter = {
		mask = int.FilterMask.Race,
		kind = int.FilterKind.Race,
		attrnames = {
			AllowedRaces = "race",
		},
	},
	RaidFilter = {
		mask = int.FilterMask.Raid,
		kind = int.FilterKind.Raid,
		attrnames = {
			AllowedFestivals = "raid",
		},
	},
	FestivalFilter = {
		mask = int.FilterMask.Festival,
		kind = int.FilterKind.Festival,
		attrnames = {
			AllowedFestivals = "festival",
		},
	},
	ScheduleFilter = {
		mask = int.FilterMask.Schedule,
		kind = int.FilterKind.Schedule,
		presence = "schedule",
		attrnames = {
			CronExpression = "schedule",
			Duration = "schedule-duration",
		},
	},
	ProfessionFilter = {
		mask = int.FilterMask.Profession,
		kind = int.FilterKind.Profession,
		attrnames = {
			AllowedProfessions = "profession",
		},
	},
	SpecializationFilter = {
		mask = int.FilterMask.Specialization,
		kind = int.FilterKind.Specialization,
		attrnames = {
			AllowedSpecializations = "specialization",
		},
	},
	--[[ShowHideModifier = {
		mask = bitop.bor(int.TriggerMask.Show, int.TriggerMask.Hide),
		getters = {
			ShowOnInteract = has_show,
			Category = show or hide,
		},
	},]]
}
for _,c in pairs(CompatNames) do
	if c.presence == nil then
		local _,p = next(c.attrnames)
		c.presence = p
	elseif c.presence == true then
		c.presence = {}
		for p,_ in pairs(c.attrnames) do
			table.insert(c.presence, p)
		end
	end
end

local IBehaviour = {
	i = {},
	mt = {}
}
local pathing_hack_autotrigger = require"@taimi/core/rt".pathing_hack_autotrigger
function IBehaviour.mt:__index(k)
	local attrname = self.b.attrnames[k]
	if attrname ~= nil then
		return rawget(self, "marker"):GetAttrByKey(attrname)
	else
		return IBehaviour.i[k]
	end
end
function IBehaviour.mt:__newindex(k, v)
	local attrname = self.b.attrnames[k]
	if attrname ~= nil then
		return rawget(self, "marker"):SetAttrByKey(attrname, v)
	else
		-- error(k, 2)
		rawset(self, k, v)
	end
end
function IBehaviour:New(marker, name, i)
	i = i or {}
	if i.name == nil then
		i.name = name
	end
	if i.b == nil then
		i.b = CompatNames[name]
	end
	if i.marker == nil then
		i.marker = marker
	end
	return util.setmetatable(i or {}, self.mt)
end
function IBehaviour.new(...)
	return IBehaviour:New(...)
end
function IBehaviour.i:Interact(auto)
	self.marker:Interact(auto)
end
function IBehaviour.i:Focus()
	self.marker:Focus()
end
function IBehaviour.i:Unfocus()
	self.marker:Unfocus()
end
function IBehaviour.i:IsFiltered()
	return self.marker.BehaviorFiltered
end
function IBehaviour.i:FilterReason()
	return self.name
end


local export = {
	TriggerKind = int.TriggerKind,
	TriggerMask = int.TriggerMask,
	CompatNames = CompatNames,
	IBehaviour = IBehaviour,
}

function export.compat_has_named(marker, name)
	local b
	if name == "ShowHideModifier" then
		b = export.compat_has_named("ShowModifier")
		if b ~= nil then
			return b
		else
			return export.compat_has_named("HideModifier")
		end
	end
	b = CompatNames[name]
	if b == nil then return nil end
	local presence
	if type(b.presence) == "table" then
		presence = ipairs
	else
		presence = util.iter_once_i
	end
	for _,pkey in presence(b.presence) do
		if marker:GetAttrByKey(pkey) ~= nil then
			return name
		end
	end
	return nil
end
function export.compat_get_named(marker, name)
	name = export.compat_has_named(marker, name)
	if name == nil then
		return nil
	end
	return IBehaviour.new(marker, name)
end
function export.iter_marker_behaviour_pairs(marker)
	local function f(_b, name)
		return export.compat_get_named(marker, name)
	end
	return util.iter_value_to_pair(
		util.iter_map_value(f, pairs(CompatNames))
	)
end

return export
