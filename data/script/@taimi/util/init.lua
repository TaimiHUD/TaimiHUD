local taimi_util = {
	getmetatable = getmetatable,
	setmetatable = setmetatable,
	rawlen = rawlen or function(t) return #t end,
}
local unpack = table.unpack or unpack
local rawlen = taimi_util.rawlen

function taimi_util.id(v) return v end
function taimi_util.typeof(v)
	local ty = type(v)
	-- TODO: recognize our builtins?
	if taimi_util.type_is_indexable(ty) then
		local typeof = v.__typeof or taimi_util.const(ty)
		ty = typeof(v)
	end
	return ty
end
function taimi_util.is_indexable(v)
	return taimi_util.type_is_indexable(type(v))
end
function taimi_util.type_is_indexable(ty)
	return ty == "table" or ty == "userdata"
end
function taimi_util.is_callable(v)
	local ty = type(v)
	-- TODO: need a rawgetmetatable_or_nil to avoid errors related to protected metatables?
	if taimi_util.type_is_indexable(ty) then
		local mt = taimi_util.getmetatable(v)
		return mt ~= nil and mt.__call ~= nil
	else return ty == "function" end
end
function taimi_util.todebugstring(v)
	local formatter
	if taimi_util.is_indexable(v) then
		formatter = v.__todebugstring
	end
	return (formatter or tostring)(v)
end

function taimi_util.metatable_of_or(out, fallback)
	local mt = taimi_util.getmetatable(out)
	if mt == nil then
		mt = fallback or {}
		taimi_util.setmetatable(out, mt)
	end
	return mt
end
function taimi_util.metatable_set_from(kv, out)
	taimi_util.table_copy_shallow(kv, taimi_util.metatable_of_or(out))
	return out
end

function taimi_util.alias_index_to(target, out)
	return taimi_util.metatable_set_from({ __index = target }, out or {})
end
function taimi_util.alias_index_chain_to(target, fallback, out)
	local index = taimi_util.fun_bind(taimi_util.table_index_or_from, fallback, target)
	return taimi_util.metatable_set_from({ __index = taimi_util.fun_skip1(index) }, out)
end

taimi_util.table = {}
if table.unpack == nil then
	taimi_util.table.unpack = unpack
end
local table_maxn = table.maxn
if table_maxn == nil then
	-- removed in lua 5.3?
	function taimi_util.table.maxn(t)
		-- TODO: metatable.__len or?
		return rawlen(t)
	end
	table_maxn = taimi_util.table.maxn or table.maxn
end
local array_move = table.move
if array_move == nil then
	function taimi_util.table.move(src, start, end_, start_dest, dest)
		dest = dest or {}
		for i=start,end_ do
			dest[start_dest] = src[i]
			start_dest = start_dest + 1
		end
		return dest
	end
	array_move = taimi_util.table.move
end
local table_pack = table.pack
if table_pack == nil then
	local pack_meta = {
		__index = function(t, k)
			if k == "n" then return table_maxn(t) else return nil end
		end,
		__len = table_maxn,
	}
	function taimi_util.table.pack(...)
		return setmetatable({...}, pack_meta)
	end
	table_pack = taimi_util.table.pack
end

function taimi_util.table_index_or_from(fallback, target, key)
	local h = target[key]
	if h == nil then
		h = fallback[key]
	end
	return h
end
function taimi_util.array_push_start(t, v)
	table.insert(t, 1, v)
end
function taimi_util.array_push_end(t, v)
	table.insert(t, v)
	--t[#t + 1] = v
	return #t
end
function taimi_util.array_remove_at(t, n)
	return table.remove(t, n)
end
function taimi_util.array_pop_end(t, n)
	n = n or #t
	local item = t[n]
	if item ~= nil then
		t[n] = nil
	end
	return item
end
function taimi_util.array_append(src, start, dest)
	return array_move(src, start, #src, taimi_util.table.maxn(dest) + 1, dest)
end
function taimi_util.array_copy_shallow(src)
	return array_move(src, 1, #src, 1)
end
function taimi_util.table_copy_shallow(kv, out)
	-- return taimi_util.table_map_collect(taimi_util.id, kv, out)
	out = out or {}
	for k, v in pairs(kv) do
		out[k] = v
	end
	return out
end
function taimi_util.table_iter_get_keys(keys, t)
	local map = function(_i, k)
		return k, t[k]
	end
	return taimi_util.iter_map2(map, ipairs(keys))
end
local function iter_once(state, cont)
	if cont ~= nil then
		return nil
	end
	return state
end
local function iter_once_i(state, cont)
	if cont ~= nil then
		return nil
	end
	return 1, state
end
function taimi_util.iter_once(s)
	return iter_once, s, nil
end
-- ipairs for a single value
function taimi_util.iter_once_i(s)
	return iter_once_i, s, nil
end
function taimi_util.table_inherit(keys, t, out)
	return taimi_util.iter_collect(out or {}, taimi_util.table_iter_get_keys(keys))
end
function taimi_util.table_map_collect(f, t, out)
	return taimi_util.iter_collect(out or {}, taimi_util.iter_map_pair(f, pairs(t)))
end
function taimi_util.table_map_each(f, t)
	return taimi_util.table_map_collect(f, t, t)
end
-- iter_chain(fun_bind1(pairs, table2nd), pairs(table1st))
function taimi_util.iter_chain(after, iter, ...)
	local iter2, s2
	local f = function(s, c)
		if iter2 == nil then
			local res = {iter(s,c)}
			if res[1] ~= nil then
				return unpack(res)
			else
				iter2, s2, c = after()
			end
		end
		return iter2(s2, c)
	end
	return f, ...
end
function taimi_util.iter_collect(out, iter, ...)
-- function taimi_util.iter_collect(out, iter, state, key_init)
	--for k, v in iter, state, key_init do
	for k, v in iter, ... do
		out[k] = v
	end
	return out
end
function taimi_util.iter_extend_array(out, iter, ...)
	out = out or {}
	for v in iter, ... do
		table.insert(out, v)
	end
	return out
end
function taimi_util.iter_map2(f, iter, state, cont)
	local map = function(s, _cont)
		local v1, v2 = iter(s, cont)
		cont = v1
		if cont ~= nil then
			v1, v2 = f(v1, v2)
		end
		return v1, v2
	end
	return map, state, cont
end
-- f(v, k)
function taimi_util.iter_map_pair(f, iter, state, cont_key)
	local map = function(s, _cont)
		local k, v = iter(s, cont_key)
		cont_key = k
		if k ~= nil then
			v, k = f(v, k)
		end
		--return k or cont_key, v, cont_key
		return k or cont_key, v
	end
	return map, state, cont_key
end
function taimi_util.iter_array_as_pair(a)
	return taimi_util.iter_value_to_pair(ipairs(a))
end
function taimi_util.iter_value_to_pair(iter, state, cont)
	local f = function(s, _cont)
		local v
		while v == nil do
			cont, v = iter(s, cont)
			if cont == nil then
				return nil
			end
		end
		return v, v
	end
	return f, state, cont
end
function taimi_util.iter_map_value(f, iter, state, cont)
	local map = function(s, cont)
		local v1, v2 = iter(s, cont)
		if v1 ~= nil then
			v2 = f(v2, v1)
		end
		return v1, v2
	end
	return map, state, cont
end
--function taimi_util.iter_map2_collect(f, out, iter, ...)
--	for k, v, k2 in iter, ... do
--		out[k] = f(v, k2 or k)
--	end
--end
function taimi_util.table_ro(t)
	return taimi_util.metatable_set_from({ __newindex = function(...) error("read-only", 2) end }, t)
end
taimi_util.alias_index_to(table, taimi_util.table)

function taimi_util.fun_bind1(f, arg1)
	return function(...) return f(arg1, ...) end
end
function taimi_util.fun_bind2(f, arg1, arg2)
	return function(...) return f(arg1, arg2, ...) end
end
function taimi_util.fun_bind(f, ...)
	local bound = table_pack(...)
	local n = bound.n
	return function(...)
		local trailing = table_pack(...)
		local bound = array_move(bound, 1, n, 1)
		array_move(trailing, 1, trailing.n, n + 1, bound)
		return f(unpack(bound))
	end
end
function taimi_util.fun_bind_method0(fname, target)
	return function(...) return target[fname](target, ...) end
end
-- function taimi_util.fun_rebind_method0(...) return taimi_util.fun_skip1(taimi_util.fun_bind_method0(...)) end
function taimi_util.fun_rebind_method0(target, fname)
	return function(_, ...) return target[fname](target, ...) end
end
function taimi_util.fun_redir_method(fname)
	return function(this, ...) return this[fname](this, ...) end
end
function taimi_util.fun_skip1(f)
	return function(_, ...) return f(...) end
end
-- function taimi_util.fun_skip_all(f) return f end
function taimi_util.fun_skip_all(f)
	return function(...) return f() end
end
function taimi_util.fun_const(v)
	return function(...) return v end
end

return taimi_util
