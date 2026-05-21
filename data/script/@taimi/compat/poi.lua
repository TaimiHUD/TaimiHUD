local ud = require"@taimi/util/ud"
local hack_autotrigger = require("@taimi/core/rt").pathing_hack_autotrigger

local Poi = ud.make_ud_wrapper(nil, "Poi")
function Poi.s.wrap(poi, pack_info)
	local i = ud.wrap_instance(Poi, poi)
	if i ~= nil then
		rawset(i, "pack_info", pack_info)
	end
	return i
end
ud.wrap_method_into(Poi, "Focus")
ud.wrap_method_into(Poi, "Unfocus")
--ud.wrap_method_into(Poi, "Interact")
ud.wrap_method_into(Poi, "Remove")
--ud.wrap_method_into(Poi, "GetBehavior")
ud.wrap_method_into(Poi, "GetAttrByKey")
ud.wrap_method_into(Poi, "SetAttrByKey")
Poi.s.attrs = {
	Alpha = "alpha",
	CanFade = "canfade",
	CullDirection = "cull",
	FadeNear = "fadenear",
	FadeFar = "fadefar",
	Guid = "guid",
	HeightOffset = "heightoffset",
	InGameVisibility = "ingamevisibility",
	MapVisibility = "mapvisibility",
	MiniMapVisibility = "minimapvisibility",
	InvertBehavior = "invertbehavior",
	MapDisplaySize = "mapdisplaysize",
	MapId = "mapid",
	MinSize = "minsize",
	MaxSize = "maxsize",
	ScaleOnMapWithZoom = "scaleonmapwithzoom",
	Size = "iconsize",
	Tint = "tint",
	TipName = "tip-name",
	TipDescription = "tip-description",
	TriggerRange = "triggerrange",
	InfoRange = "inforange",
	ResetLength = "resetlength",
	AutoTrigger = "autotrigger",
	-- Position = "position",
	-- RotationXyz = "rotate",
}
-- TODO: source from a core/builtin
Poi.s.tag_type = 1
function Poi.mt.__index(t, k)
	local attr = Poi.attrs[k]
	if attr ~= nil then
		return rawget(t, ud.key_instance):GetAttrByKey(attr)
	elseif k == "Behaviors" then
		return t:GetBehaviors()
	elseif k == "Category" then
		return t:GetCategory()
	elseif k == "Texture" then
		return t:GetTexture()
	elseif k == "Position" then
		return rawget(t, ud.key_instance).Position:xzy()
	elseif k == "DistanceToPlayer" then
		return t:GetDistanceToPlayer()
	elseif k == "Category" then
		local cat = rawget(t, ud.key_instance):GetAttrByKey("type")
		if type(cat) == "string" then
			cat = self.pack_info:CategoryByType(cat)
		end
		if cat ~= nil then
			local Category = require("@taimi/compat/category").Category
			cat = Category.wrap(cat, self.pack_info)
		end
		return cat
	end
	local v = ud.instance_mt.__index(t, k)
	if v ~= nil and k == "Texture" and type(v) == "string" then
		v = t.pack_info:GetPackAssets():OpenTexture(v)
	end
	return v
end
function Poi.mt.__newindex(t, k, v)
	local attr = Poi.attrs[k]
	-- TODO: lookup table for accessor fns
	if attr ~= nil then
		return rawget(t, ud.key_instance):SetAttrByKey(attr, v)
	elseif k == "Texture" then
		t:SetTexture(v)
	elseif k == "Position" then
		t:SetPos(v)
	elseif k == "RotationXyz" then
		t:SetRot(v)
	elseif k == "DistanceToPlayer" then
		-- what does this even do, lerp the marker position?
		error("why? report your usecase if needed", 2)
	end
	return ud.instance_mt.__newindex(t, k, v)
end
function Poi.i:SetPosX(v)
	self:SetAttrByKey("xpos", v)
end
function Poi.i:SetPosY(v)
	-- blishspace
	self:SetAttrByKey("zpos", v)
end
function Poi.i:SetPosZ(v)
	-- blishspace
	self:SetAttrByKey("ypos", v)
end
function Poi.i:SetRotX(v)
	self:SetAttrByKey("rotate-x", v)
end
function Poi.i:SetRotY(v)
	self:SetAttrByKey("rotate-y", v)
end
function Poi.i:SetRotZ(v)
	self:SetAttrByKey("rotate-z", v)
end
function Poi.i:SetPos(x, ...)
	if type(x) == "number" then
		local Vec3 = require("@taimi/core/vectors").Vec3
		x = Vec3(x, ...)
	end
	--[[self:SetPosX(x.X)
	self:SetPosY(x.Y)
	self:SetPosZ(x.Z)]]

	--[[ blishspace...
	local z = x.Y
	x.Y = x.Z
	x.Z = z]]
	x = x:xzy()

	ud.unwrap(self):SetPos(x)
	if hack_autotrigger and self:GetAttrByKey("autotrigger") then
		if self:hack_autofocus() then
			self:Focus()
			self:Interact(true)
		end
	elseif hack_autotrigger and (self:GetAttrByKey("script-focus") or self:GetAttrByKey("info")) then
		if self:hack_autofocus() then
			self:Focus()
		end
	end
end
function Poi.i:SetRot(x, ...)
	if type(x) == "number" then
		local Vec3 = require("@taimi/core/vectors").Vec3
		x = Vec3(x, ...)
	end
	--[[self:SetRotX(x.X)
	self:SetRotY(x.Y)
	self:SetRotZ(x.Z)]]
	ud.unwrap(self):SetRot(x)
end
function Poi.i:Remove()
	self.pack_info:GetPackHandle():RemoveMarker(ud.unwrap(self))
end
function Poi.i:SetTexture(tex)
	if type(tex) == "number" then
		self:SetWebTexture(tex)
	else
		-- TODO: ud.unwrap(self):SetTexture(tex)
		self:SetAttrByKey("iconfile", tostring(tex))
	end
end
function Poi.i:SetWebTexture(id)
	error("unimplemented: SetWebTexture")
end
function Poi.i:GetTexture()
	local tex = self[ud.key_instance]:GetAttrByKey("iconfile")
	if type(tex) == "string" then
		tex = self.pack_info:GetPackAssets():OpenTexture(tex)
	end
	-- if tex ~= nil then tex = Texture.wrap(tex, self.pack_info) end
	return tex
end
function Poi.i:GetCategory()
	local cat = self[ud.key_instance]:GetAttrByKey("type")
	if type(cat) == "string" then
		cat = self.pack_info:CategoryByType(cat)
	end
	if cat ~= nil then
		local Category = require("@taimi/compat/category").Category
		cat = Category.wrap(cat, self.pack_info)
	end
	return cat
end
function Poi.i:GetBehavior(name)
	local int = require"@taimi/pack/interact"
	return int.compat_get_named(self, name)
end
function Poi.i:GetBehaviors()
	local int = require"@taimi/pack/interact"
	return util.iter_extend_array({}, int.iter_marker_behaviour_pairs(self))
end
function Poi.i:GetDistanceToPlayer()
	return self.pack_info:GetSpaceHandle():GetDistanceToPlayer(ud.unwrap(self))
end
function Poi.i:Interact(auto, ...)
	if require("@taimi/core/rt").pathing_hack_interact and (auto or false) == (self:GetAttrByKey("autotrigger") or false) then
		-- TODO: if self:GetAttrByKey("script-trigger") ~= nil then eventloop.queue() end
		--[[local info = self:GetAttrByKey("info")
		if info ~= nil then
			local uix = require"@taimi/core/ui/exchange"
			uix.info_notify(info)
		end]]
		local copy = self:GetAttrByKey("copy")
		if copy ~= nil then
			local uix = require"@taimi/core/ui/exchange"
			uix.clipboard_send(copy)
			local msg = self:GetAttrByKey("copy-message")
			if msg ~= nil then
				uix.info_notify(msg)
			end
		end
		if --[[info or]] copy then
			return
		end
	end
	return ud.unwrap(self):Interact(auto, ...)
end
--[[function Poi.i:Focus(...)
	if require("@taimi/core/rt").pathing_hack_interact then
		-- TODO: if self:GetAttrByKey("script-focus") ~= nil then eventloop.queue() end
		rawset(self, "Focused", true)
	end
	return ud.unwrap(self):Focus(...)
end
function Poi.i:Unfocus(...)
	if require("@taimi/core/rt").pathing_hack_interact then
		rawset(self, "Focused", nil)
	end
	return ud.unwrap(self):Unfocus(...)
end]]

function Poi.i:hack_autofocus()
	local range = self:GetAttrByKey("triggerrange") or self:GetAttrByKey("inforange") or 2.0
	-- local dist = (require("@taimi/core/mumblelink").Mumble.PlayerCharacter.Position - self.Position):Length()
	return not rawget(self, "Focused") and self.DistanceToPlayer <= range
end

return {
	Poi = Poi,
}
