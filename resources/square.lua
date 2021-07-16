SONGMAP = {}
CURR_GROUP = 0

-- "dump table" function from https://stackoverflow.com/a/27028488
-- modified slightly to add linebreaks
function dump(o)
   if type(o) == 'table' then
      local s = '{'
      for k,v in pairs(o) do
         if type(k) ~= 'number' then k = '"'..k..'"' end
         s = s .. '['..k..'] = ' .. dump(v) .. ','
      end
      return s .. '}\n'
   else
      return tostring(o)
   end
end

function add_action(beat, group, action)
   action["beat"] = beat
   action["enemygroup"] = group
   table.insert(SONGMAP, action)
end

function bullet(start_pos, end_pos)
   return {spawn_cmd = "bullet", start_pos = start_pos, end_pos = end_pos}
end

function pos(x, y)
   return {x = x, y = y}
end


function lerp(a, b, t)
   return a * (1.0 - t) + b * t
end

function lerp_pos(a, b, t)
   return {x = lerp(a.x, b.x, t), y = lerp(a.y, b.y, t)}
end

--- Beat splitter iterator
-- Create a "beat splitter" which returns the following things:
-- 1. beat - the beattime, as a float, that the beat occurs on
-- 2. percent - the percent over the total duration
-- @param start The time to start at. 
-- @param duration The length of time that beats will be yielded between.
-- @param frequency The length of time between each beat.
-- @param offset The amount to shift every beat. This will also alter the
--               percent value.
-- @param delay The amount to shift every beat. This will NOT alter the percent
--              value.
-- Note that if offset or delay are non-zero, then the first returned beat might
-- not occur at `start`.
function beat_splitter(start, duration, frequency, offset, delay)
   local offset = offset or 0.0
   local delay = delay or 0.0
   local this_beat = start
   return function ()
      if duration > this_beat - start then
         local beat = this_beat + delay + offset
         local percent = (this_beat + offset - start) / duration
         this_beat = this_beat + frequency
         return {beat = beat, percent = percent}
      else
         return nil
      end
   end
end

-- convience function for beat splitters
function every4(start)
   return beat_splitter(start, 16.0, 4.0, 0.0, 0.0)
end

function every2(start)
   return beat_splitter(start, 16.0, 2.0, 0.0, 0.0)
end

function every1(start)
   return beat_splitter(start, 16.0, 1.0, 0.0, 0.0)
end

--- Adds beat actions to SONGMAP using the given beat iterator and spawner.
-- @param beat_iter - A beat iterator. This is expected to return a table with 
--                    the following fields:
--                    beat - a float of the beattime
--                    percent - a float of the percent over the duration
--                    pitch - (optional) the absolute normalized pitch value of
--                            the note. If not present, this will default to 0.0.
-- @param spawner - A function returning spawn_cmd tables. This function is expected to
--                  beat, percent, and pitch in that order.
-- Note that the enemygroup will be CURR_GROUP and the start time will be the
-- beattime given by beat_iter
function make_actions(beat_iter, spawner)
   for marked_beat in beat_iter do
      local beat = marked_beat.beat
      local percent = marked_beat.percent
      local pitch = marked_beat.pitch or 0.0
      local spawn_cmd = spawner(beat, percent, pitch)
      add_action(beat, CURR_GROUP, spawn_cmd)
   end
end

function bullet_lerp(start1, end1, start2, end2)
   return function(beat, t)
      local start_pos = lerp_pos(start1, start2, t)
      local end_pos = lerp_pos(end1, end2, t)
      return bullet(start_pos, end_pos)
   end
end


-- Position constants
ORIGIN = pos(0.0, 0.0)
TOPLEFT = pos(-50.0, 50.0)
BOTLEFT = pos(-50.0, -50.0)
TOPRIGHT = pos(50.0, 50.0)
BOTRIGHT = pos(50.0, -50.0)


-- Set up BPM, amount of song to skip, etc
table.insert(SONGMAP, {bpm = 150.0})
table.insert(SONGMAP, {skip = 0.0 * 4.0})



-- Song data

-- Measures 4 - 7 (beats 16)


make_actions(every4(4.0 * 4.0), bullet_lerp(BOTLEFT, ORIGIN, BOTRIGHT, ORIGIN))
make_actions(every4(4.0 * 4.0), bullet_lerp(TOPRIGHT, ORIGIN, TOPLEFT, ORIGIN))


-- Measures 8 - 11 (beat 32)
make_actions(every2(8.0 * 4.0), bullet_lerp(TOPLEFT, ORIGIN, BOTLEFT,  ORIGIN))
make_actions(every2(8.0 * 4.0), bullet_lerp(BOTRIGHT, ORIGIN, TOPRIGHT, ORIGIN))

-- Measures 12 - 15 (beat 48)
every2offset = beat_splitter(12.0 * 4.0, 16.0, 2.0, 1.0, 0.0)
make_actions(every2(12.0 * 4.0), bullet_lerp(TOPRIGHT, TOPLEFT, BOTRIGHT, BOTLEFT))
make_actions(every2offset, bullet_lerp(BOTLEFT, BOTRIGHT, TOPLEFT, TOPRIGHT))

-- Measures 16 - 19 (beat 64)
-- make_actions(buildup1main2, bullet_player());









return SONGMAP
