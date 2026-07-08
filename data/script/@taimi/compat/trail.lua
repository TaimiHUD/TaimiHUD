local ud = require"@taimi/util/ud"

local Trail = ud.make_ud_wrapper(nil, "Trail")
function Trail.s.wrap(trail, pack_info)
	local i = ud.wrap_instance(Trail, trail)
	if i ~= nil then
		rawset(i, "pack_info", pack_info)
	end
	return i
end
ud.wrap_method_into(Trail, "Focus")
ud.wrap_method_into(Trail, "Unfocus")
ud.wrap_method_into(Trail, "Interact")
-- ud.wrap_method_into(Trail, "GetBehavior")
ud.wrap_method_into(Trail, "GetAttrByKey")
ud.wrap_method_into(Trail, "SetAttrByKey")
Trail.s.attrs = {
	Alpha = "alpha",
	AnimationSpeed = "animspeed",
	CanFade = "canfade",
	CullDirection = "cull",
	FadeNear = "fadenear",
	FadeFar = "fadefar",
	Guid = "guid",
	IsWall = "iswall",
	InGameVisibility = "ingamevisibility",
	MapVisibility = "mapvisibility",
	MiniMapVisibility = "minimapvisibility",
	MapId = "mapid",
	Tint = "tint",
	TrailSampleColor = "map-tint",
	TrailScale = "trailscale",
	-- TriggerRange = "triggerrange",
}
-- TODO: source from a core/builtin
Trail.s.tag_type = 2
function Trail.mt.__index(t, k)
	local attr = Trail.attrs[k]
	if attr ~= nil then
		return rawget(t, ud.key_instance):GetAttrByKey(attr)
	elseif k == "Behaviors" then
		return t:GetBehaviors()
	elseif k == "Category" then
		return t:GetCategory()
	elseif k == "Texture" then
		return t:GetTexture()
	end
	local v = ud.instance_mt.__index(t, k)
	if v ~= nil and k == "Texture" and type(v) == "string" then
		v = t.pack_info:GetPackAssets():OpenTexture(v)
	end
	return v
end
function Trail.mt.__newindex(t, k, v)
	local attr = Trail.attrs[k]
	if attr ~= nil then
		rawget(t, ud.key_instance):SetAttrByKey(attr, v)
	elseif k == "Texture" then
		t:SetTexture(v)
	else
		ud.instance_mt.__newindex(t, k, v)
	end
end
function Trail.i:Remove()
	self.pack_info:GetPackHandle():RemoveTrail(ud.unwrap(self))
end
function Trail.i:SetTexture(tex)
	if type(tex) == "number" then
		self:SetWebTexture(tex)
	else
		-- TODO: if type(tex) == "string" then tex = self.pack_info:GetPackAssets():OpenTexture(tex) end
		-- TODO: ud.unwrap(self):SetTexture(tex)
		self:SetAttrByKey("texture", tostring(tex))
	end
end
function Trail.i:SetWebTexture(id)
	error("unimplemented: SetWebTexture")
end
function Trail.i:GetTexture()
	local tex = self[ud.key_instance]:GetAttrByKey("texture")
	if type(tex) == "string" then
		tex = self.pack_info:GetPackAssets():OpenTexture(tex)
	end
	-- if tex ~= nil then tex = Texture.wrap(tex, self.pack_info) end
	return tex
end
function Trail.i:GetCategory()
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
function Trail.i:GetBehavior(name)
	local int = require"@taimi/pack/interact"
	return int.compat_get_named(self, name)
end
function Trail.i:GetBehaviors()
	local int = require"@taimi/pack/interact"
	return util.iter_extend_array({}, int.iter_marker_behaviour_pairs(self))
end

return {
	Trail = Trail,
}
