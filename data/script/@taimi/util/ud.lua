local util = require"@taimi/util"
local ud = {
	key_instance = {},
	key_name = {},
	key_clz = {},
	registrations = {},
	instance_mt = {},
	instance = {},
}
local key_instance = ud.key_instance
local key_clz = ud.key_clz
local key_name = ud.key_name

local unwrap
function ud.unwrap(v, o)
	local ty = type(v)
	if ty == "userdata" then return v
	elseif ty == "nil" then return o
	end
	return unwrap(v[key_instance], v)
end
unwrap = ud.unwrap
local unwrap_any
function ud.unwrap_any(v, o)
	local ty = type(v)
	if ty == "table" then return unwrap_any(v[key_instance], v)
	elseif ty == "nil" then return o
	-- elseif ty == "userdata" then return v
	else return v
	end
end
unwrap_any = ud.unwrap_any

function ud.instance_mt.__index(t, k)
	local v = rawget((rawget(t, key_clz) or t[key_clz]), "i")[k]
	if v ~= nil then
		return v
	else
		return (rawget(t, key_instance) or t[key_instance])[k]
	end
end
function ud.instance_mt.__newindex(t, k, v)
	-- TODO? if rawget(t, k) ~= nil then return rawset(t, k, v) end
	t[key_instance][k] = v
end
function ud.instance_mt.__tostring(t)
	local instance = rawget(t, key_instance) or t[key_instance]
	if instance == nil then
		instance = rawget(t, key_clz) or t[key_clz]
		if instance ~= nil then
			instance = instance[key_name]
		end
	end
	if instance ~= nil then
		return tostring(instance)
	else
		return "userdata"
	end
end
function ud.instance_mt.__call(t, ...)
	local instance = rawget(t, key_instance) or t[key_instance]
	if instance == nil then
		error("not ud", 2)
	end
	return instance(...)
end
function ud.instance_mt.__len(t)
	local instance = rawget(t, key_instance) or t[key_instance]
	if instance == nil then
		error("not ud", 2)
	end
	return #instance
end
function ud.instance_mt.__unm(t)
	local instance = rawget(t, key_instance) or t[key_instance]
	if instance == nil then
		error("not ud", 2)
	end
	return -instance
end
-- TODO: ipairs?
function ud.instance_mt.__concat(l, r)
	return unwrap_any(l) .. unwrap_any(r)
end
function ud.instance_mt.__eq(l, r)
	return unwrap_any(l) == unwrap_any(r)
end
function ud.instance_mt.__lt(l, r)
	return unwrap_any(l) < unwrap_any(r)
end
function ud.instance_mt.__le(l, r)
	return unwrap_any(l) <= unwrap_any(r)
end
function ud.instance_mt.__add(l, r)
	return unwrap_any(l) + unwrap_any(r)
end
function ud.instance_mt.__sub(l, r)
	return unwrap_any(l) - unwrap_any(r)
end
function ud.instance_mt.__mul(l, r)
	return unwrap_any(l) * unwrap_any(r)
end
function ud.instance_mt.__div(l, r)
	return unwrap_any(l) / unwrap_any(r)
end
function ud.instance_mt.__mod(l, r)
	return unwrap_any(l) % unwrap_any(r)
end
function ud.instance_mt.__pow(l, r)
	return unwrap_any(l) ^ unwrap_any(r)
end

function ud.make_ud_wrapper(c_clazz, name, clz)
	clz = clz or {}
	clz[ud.key_name] = name
	if c_clazz == nil then
		clz.c_clazz = {}
	else
		clz.c_clazz = c_clazz
		ud.registrations[name] = clz
	end
	clz.i = util.table_copy_shallow(ud.instance, clz.i or {})
	if clz.s == nil then
		clz.s = {}
	end
	clz.mt = util.table_copy_shallow(ud.instance_mt, clz.mt or {})
	if clz.clz_mt == nil then
		clz.clz_mt = {}
	end
	function clz.clz_mt.__index(t, k)
		local v = rawget(t, "s")[k]
		if v ~= nil then
			return v
		else
			return rawget(t, "c_clazz")[k]
		end
	end
	function clz.clz_mt.__newindex(t, k, v)
		rawget(t, "c_clazz")[k] = v
	end
	return util.setmetatable(clz, clz.clz_mt)
end
local wrap_instance
function ud.wrap_instance(clz, c_i)
	if c_i == nil then return nil end
	local i = util.setmetatable({
		[key_instance] = c_i,
		[key_clz] = clz,
	}, clz.mt)
	return i
end
wrap_instance = ud.wrap_instance
function ud.wrap_constructor_into(clz, name)
	local f = function(...)
		return wrap_instance(clz, clz.c_clazz[name](...))
	end
	rawset(clz, name, f)
	return f
end
function ud.wrap_static_method_into(clz, name)
	local f = function(this, ...)
		return clz.c_clazz[name](unwrap(this), ...)
	end
	rawset(clz, name, f)
	return f
end
function ud.wrap_method_into(clz, name)
	local f = function(this, ...)
		local i = this[key_instance]
		return i[name](unwrap(i), ...)
	end
	rawset(clz.i, name, f)
	return f
end
function ud.c_instance_of(i)
	-- TODO: just alias to unwrap?
	return i[key_instance]
end
function ud.c_static_of(i)
	return i[key_clz].c_clazz
end
function ud.static_of(i)
	return i[key_clz]
end

return ud
