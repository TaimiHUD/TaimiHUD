local util = require"@taimi/util"

local taimi_log = require"@taimi/core/log"
local function debug_prints()
	return util.iter_map_pair(util.fun_skip1,
		util.table_iter_get_keys({"Info", "Warn", "Error", "Print"}, taimi_log)
	)
end

-- globals expected to be available to pack scripts
local Context = {}
Context.env = {
	Debug = util.iter_collect({}, debug_prints()),
}
Context.env.print = taimi_log.Print

local version = require"@taimi/core/version"
Context.env.Pathing = {}
Context.env.Pathing.IsVersionAtLeast = util.fun_rebind_method0(version.Compat, "IsVersionAtLeast")
Context.env.Pathing.Version = tostring(version.Compat:Scrub())
Context.env.PathingVersion = tostring(version.Compat)
Context.env.TaimiVersion = tostring(version.Taimi)
Context.env.TaimiApiVersion = tostring(version.Api)

local mumblelink = require"@taimi/core/mumblelink"
Context.env.Mumble = mumblelink.Mumble

local vectors = require"@taimi/core/vectors"
Context.env.I = {
	Vector3 = util.fun_skip1(vectors.Vec3),
	Color = util.fun_skip1(vectors.Colour),
	Guid = util.fun_skip1(vectors.Guid),
}
function Context.env.I:WebTexture(id)
	error("unimplemented: WebTexture")
end

Context.user_alerts = {}
Context.env.User = {}
function Context.env.User:SetClipboard(value, message)
	local uix = require("@taimi/ui/exchange")
	uix.Clipboard.Send(value)
	uix.Alert.NotifyClipboard(value, message)
end
function Context.env.User:ShowInfo(message)
	local alert = require("@taimi/ui/exchange").Alert.new(message)
	alert:Show()
	local idx = util.array_push_end(Context.user_alerts, alert)
	return tostring(idx)
end
function Context.env.User:HideInfo(key)
	key = tonumber(key)
	if key and Context.user_alerts[key] then
		Context.user_alerts[key]:Hide()
		Context.user_alerts[key] = false
	end
end

function Context.event_for_pack(Taimi)
	local Event = {}
	function Event:OnTick(cb)
		local event = require"@taimi/event"
		local function f(_eloop, _e, ...)
			cb(...)
		end
		Taimi.ctx.events:RegisterFunc(event.HostSignal.PathingTick, f)
	end
	return Event
end
function Context.debug_for_pack(Taimi)
	local taimi_debug = require"@taimi/debug"
	local debug_watch = taimi_debug.Debug.new()
	local Debug = util.alias_index_to(Context.env.Debug,
		util.iter_collect({
			watches = debug_watch,
		}, util.iter_map_value(util.fun_bind1(util.fun_rebind_method0, debug_watch),
			util.iter_array_as_pair({"Watch", "ClearWatch"})
		))
	)
	return Debug
end
function Context.storage_for_pack(Taimi)
	local Storage = {}
	local storage = function()
		if Taimi.ctx.storage == nil then
			Taimi.ctx.storage = Taimi.ctx.plug:GetStorage()
		end
		return Taimi.ctx.storage
	end
	function Storage:UpsertValue(...)
		return storage():InsertString(...)
	end
	function Storage:DeleteValue(...)
		storage():RemoveKey(...)
	end
	function Storage:ReadValue(...)
		return storage():GetString(...)
	end
	return Storage
end

function Context.pack_for_pack(pack_info_key, Pack, genv)
	local function require_hacks(genv, assets, src)
		local ok, res
		if src == "Scripts/credits.lua" and genv.MMM ~= nil and rawget(genv, "Teh") == nil then
			-- hack around broken copy-pasted script in Metal-Marker-Myriad
			ok = true
			rawset(genv, "Teh", genv.MMM)
			res = assets:Require(src)
			rawset(genv, "Teh", nil)
		elseif src == "scripts/Utility/Throw Helper" and genv.HMP ~= nil then
			ok, res = pcall(assets.Require, assets, "scripts/General/Throw Helper.lua")
		elseif src == "Data/TehsTrails/Scripts/skyscaleinfo.lua" and genv.Teh ~= nil then
			-- Teh.triggerRangeReduced does not exist and breaks a condition guard
			-- causes marker+prefs updates every tick if not fixed
			ok = true
			res = assets:Require(src)
			if genv.Teh.skyscale ~= nil then
				local typo_mt = {}
				function typo_mt:__index(k)
					if k == "triggerRangeReduced" then
						return rawget(self, "skyscale")[k]
					end
				end
				setmetatable(genv.Teh, typo_mt)
			end
		end
		return ok, res
	end
	function Pack:Require(...)
		local assets = self[pack_info_key]:GetPackAssets()
		if genv ~= nil then
			local ok, res = require_hacks(genv, assets, ...)
			if ok then
				return res
			end
		end
		return assets:Require(...)
	end
	local lateattrs = {
		-- hack in attrs that can't currently be set on MarkerAttributes...
		xpos = true,
		ypos = true,
		zpos = true,
		position = true,
		mapid = true,
		type = true,
		guid = true,
		traildata = true,
		trailsamplecolor = true
	}
	function Pack:CreateMarker(attrs)
		local ud = require("@taimi/util/ud")
		local MarkerAttributes = require("@taimi/pack/attrs").MarkerAttributes

		local a = MarkerAttributes.new_poi()
		local lateset = {}
		for k,v in pairs(attrs) do
			if lateattrs[string.lower(k)] then
				lateset[k] = v
			else
				a:SetAttr(k, v)
			end
		end

		--[[ hack for pre-interact branch...
		local info_msg = a:GetAttrByKey("info")
		if info_msg ~= nil and a:GetAttrByKey("autoTrigger") == true then
			wrap as ShowInfo() dummy or something... wait for SetPos() to place it in range though!
		end]]

		local Poi = require("@taimi/compat/poi").Poi
		local p = self[pack_info_key]:GetPackHandle():CreateMarker(ud.unwrap(a))
		for k,v in pairs(lateset) do
			p:SetAttrByKey(k, v)
		end
		if p:GetAttrByKey("mapid") == nil then
			local Mumble = require"@taimi/core/mumblelink".Mumble
			if Mumble.IsAvailable and Mumble.CurrentMap.Id ~= nil then
				p:SetAttrByKey("mapid", Mumble.CurrentMap.Id)
			end
		end
		local poi = Poi.wrap(p, self[pack_info_key])
		if require("@taimi/core/rt").pathing_hack_autotrigger and attrs["xpos"] and poi:GetAttrByKey("autotrigger") then
			if poi:hack_autofocus() then
				poi:Interact(true)
			end
		end
		return poi
	end
	function Pack:CreateTrail(attrs)
		local ud = require("@taimi/util/ud")
		local MarkerAttributes = require("@taimi/pack/attrs").MarkerAttributes
		local a = MarkerAttributes.new_trail()
		local lateset = {}
		for k,v in pairs(attrs) do
			if lateattrs[string.lower(k)] then
				lateset[k] = v
			else
				a:SetAttr(k, v)
			end
		end
		local Trail = require("@taimi/compat/trail").Trail
		local t = self[pack_info_key]:GetPackHandle():CreateTrail(ud.unwrap(a))
		for k,v in pairs(lateset) do
			t:SetAttrByKey(k, v)
		end
		if t:GetAttrByKey("mapid") == nil then
			local Mumble = require"@taimi/core/mumblelink".Mumble
			if Mumble.IsAvailable and Mumble.CurrentMap.Id ~= nil then
				t:SetAttrByKey("mapid", Mumble.CurrentMap.Id)
			end
		end
		return Trail.wrap(t, self[pack_info_key])
	end
	return Pack
end
function Context.env_for_plug(pack_info, genv, out, pack_info_key)
	out = out or {}
	pack_info_key = pack_info_key or {}
	local event = require"@taimi/event"
	local Taimi = out.Taimi or require"@taimi/v0".new_plug(pack_info)
	-- TODO? Taimi.ctx.genv = genv
	-- TODO? ctx.globals = genv._G,
	-- TODO? ctx.userdata = {},
		--[[Pack = packs.Pack.new(pack_info, {
			Require = util.fun_rebind_method0(pack_info, "Require"),
		}),]]
	local pack_info_key = {}
	out = util.table_copy_shallow(Context.env, out)

	local function table_index(self, k)
		local v = util.table[k]
		if v ~= nil then
			-- nothing
		elseif k == "ToLson" or k == "FromLson" then
			v = require"@taimi/todo/lson"[k]
		end
		rawset(self, k, v)
		return v
	end
	out.table = util.setmetatable({}, {
		__index = table_index,
		__metatable = {},
	})
	-- util.table_ro(out.table)
	-- TODO? util.table_copy_shallow(util.table, out.table)

	out.Taimi = Taimi
	out.Debug = Context.debug_for_pack(Taimi)
	out.Event = Context.event_for_pack(Taimi)
	out.Storage = Context.storage_for_pack(Taimi)
	out.Pack = Context.pack_for_pack(pack_info_key, {
		[pack_info_key] = Taimi.ctx.plug,
	}, genv)
	local menus = require"@taimi/compat/menu"
	-- TODO: out.World = all packs?
	out.Menu = menus.RootMenu.new(Taimi.ctx.plug, Taimi.ctx.events)
	out.I = util.alias_index_to(out.I, {})
	function out.I:Texture(a, ...)
		if type(a) == "number" then
			return out.I:WebTexture(a, ...)
		else
			local pack = a
			return pack[pack_info_key]:GetPackAssets():OpenTexture(...)
		end
	end
	return out
end
function Context.env_for_pack(pack_info, genv, out)
	local pack_info_key = {}
	out = Context.env_for_plug(pack_info, genv, out, pack_info_key)
	-- local packs = require"@taimi/core/packs"
	local Taimi = out.Taimi
	local cats = require"@taimi/compat/category"
	out.Category = cats.RootCategory.new(Taimi.ctx.plug)
	local trails = require"@taimi/compat/trail"
	local pois = require"@taimi/compat/poi"
	local menus = require"@taimi/compat/menu"
	out.World = {
		[pack_info_key] = Taimi.ctx.plug,
		GetClosestMarker = function(t, ...) return t[pack_info_key]:GetSpaceHandle():GetClosestMarker(...) end,
		GetClosestMarkers = function(t, ...) return t[pack_info_key]:GetSpaceHandle():GetClosestMarkers(...) end,
	}
	local function lookup_map_filter(pack_info)
		local Mumble = require"@taimi/core/mumblelink".Mumble
		if Mumble.IsAvailable then
			return Mumble.CurrentMap.Id
		else
			return nil
		end
	end
	function out.World:PathablesByGuid(guid)
		local pack_info = self[pack_info_key]
		local markers = pack_info:GetWorldHandle():PathablesByGuid(guid, lookup_map_filter(pack_info))
		if markers ~= nil then
			for i,marker in ipairs(markers) do
				local targetty = marker.PathableTagType
				if targetty == pois.Poi.tag_type then
					markers[i] = pois.Poi.wrap(marker)
				elseif targetty == trails.Trail.tag_type then
					markers[i] = trails.Trail.wrap(marker)
				else
					-- TODO: if rt.is_debug_idk then error("unrecognized marker type tag") end
				end
			end
		end
		return markers
	end
	function out.World:PathableByGuid(guid)
		local pack_info = self[pack_info_key]
		local marker = pack_info:GetWorldHandle():PathableByGuid(guid, lookup_map_filter(pack_info))
		if marker ~= nil then
			local targetty = marker.PathableTagType
			if targetty == pois.Poi.tag_type then
				marker = pois.Poi.wrap(marker)
			elseif targetty == trails.Trail.tag_type then
				marker = trails.Trail.wrap(marker)
			else
				-- TODO: if rt.is_debug_idk then error("unrecognized marker type tag") end
			end
		end
		return marker
	end
	function out.World:TrailByGuid(guid)
		local pack_info = self[pack_info_key]
		local trail = pack_info:GetWorldHandle():TrailByGuid(guid, lookup_map_filter(pack_info))
		if trail ~= nil then
			trail = trails.Trail.wrap(trail, pack_info)
		end
		return trail
	end
	function out.World:MarkerByGuid(guid)
		local pack_info = self[pack_info_key]
		local poi = pack_info:GetWorldHandle():MarkerByGuid(guid, lookup_map_filter(pack_info))
		if poi ~= nil then
			poi = pois.Poi.wrap(poi, pack_info)
		end
		return poi
	end
	function out.World:CategoryByType(...)
		local pack_info = self[pack_info_key]
		local cat = pack_info:CategoryByType(...)
		if cat ~= nil then
			cat = cats.Category.wrap(cat, pack_info)
		end
		return cat
	end
	out.I.Marker = util.fun_rebind_method0(out.Pack, "CreateMarker")
	out.I.Trail = util.fun_rebind_method0(out.Pack, "CreateTrail")
	-- deprecated alias
	out.Packs = out.World
	return out
end

return Context
