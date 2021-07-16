---@diagnostic disable: lowercase-global
SONGMAP = {}
CURR_GROUP = 0

-- "dump table" function from https://stackoverflow.com/a/27028488
-- modified slightly to add linebreaks
function dump(o)
    if type(o) == 'table' then
        local s = '{'
        for k, v in pairs(o) do
            if type(k) ~= 'number' then
                k = '"' .. k .. '"'
            end
            s = s .. '[' .. k .. '] = ' .. dump(v) .. ','
        end
        return s .. '}\n'
    else
        return tostring(o)
    end
end

-- "deep copy" function from http://lua-users.org/wiki/CopyTable
function deepcopy(orig)
    local orig_type = type(orig)
    local copy
    if orig_type == 'table' then
        copy = {}
        for orig_key, orig_value in next, orig, nil do
            copy[deepcopy(orig_key)] = deepcopy(orig_value)
        end
        setmetatable(copy, deepcopy(getmetatable(orig)))
    else -- number, string, boolean, etc
        copy = orig
    end
    return copy
end

-- return a if true, b if false
function ternary(cond, a, b)
    if cond then
        return a
    else
        return b
    end
end

-- Creates a deepcopy of the marked_beats array and offsets each beat by `offset`
function add_offset(marked_beats, offset)
    local marked_beats = deepcopy(marked_beats)
    for index, marked_beat in ipairs(marked_beats) do
        marked_beat.beat = marked_beat.beat + offset
    end
    return marked_beats
end

-- Creates a deepcopy of the marked_beats array and normalizes the pitches of each
-- beat. Entries without a "pitch" field are skipped.
function normalize_pitch(marked_beats)
    local marked_beats = deepcopy(marked_beats)
    local min = 999.0
    local max = -999.0
    for _, beat in ipairs(marked_beats) do
        if beat.pitch then
            if beat.pitch > max then
                max = beat.pitch
            end
            if beat.pitch < min then
                min = beat.pitch
            end
        end
    end

    if (not min) or (not max) or (min == max) then
        return marked_beats
    end

    for _, beat in ipairs(marked_beats) do
        if beat.pitch then
            beat.pitch = (beat.pitch - min) / (max - min)
        end
    end
    return marked_beats
end

-- Add an spawn_cmd to the songmap.
function add_action(beat, group, action)
    action["beat"] = beat
    action["enemygroup"] = group
    table.insert(SONGMAP, action)
end

-- Add an spawn_cmd to the songmap. If beat or enemygroup are already present in
-- action, then the passed in argument is not used
function add_action_overridable(beat, group, action)
    if not action["beat"] then
        action["beat"] = beat
    end

    if not action["enemygroup"] then
        action["enemygroup"] = group
    end
    table.insert(SONGMAP, action)
end

function bullet(start_pos, end_pos)
    return {
        spawn_cmd = "bullet",
        start_pos = start_pos,
        end_pos = end_pos
    }
end

function laser_angle(position, angle)
    return {
        spawn_cmd = "laser",
        position = position,
        angle = angle
    }
end

function laser_points(a, b)
    return {
        spawn_cmd = "laser",
        a = a,
        b = b
    }
end

function set_rotation_on(start_angle, end_angle, duration, rotation_point)
    return {
        spawn_cmd = "set_rotation_on",
        start_angle = start_angle,
        end_angle = end_angle,
        duration = duration,
        rot_point = rotation_point
    }
end

function set_rotation_off()
    return {
        spawn_cmd = "set_rotation_off"
    }
end

function pos(x, y)
    return {
        x = x,
        y = y
    }
end

function lerp(a, b, t)
    return a * (1.0 - t) + b * t
end

function lerp_pos(a, b, t)
    return {
        x = lerp(a.x, b.x, t),
        y = lerp(a.y, b.y, t)
    }
end

function durations(warmup, active, cooldown)
    return {
        warmup = warmup,
        active = active,
        cooldown = cooldown
    }
end

-- Return a position about some circle. Angle is in degrees
function circle(center, radius, angle)
    local angle = math.rad(angle)
    local x = center.x + math.cos(angle) * radius
    local y = center.y + math.sin(angle) * radius
    return pos(x, y)
end

--- Beat splitter
-- Creates an array of marked beats with the following fields:
-- beat - the beattime, as a float, that the beat occurs on
-- percent - the percent over the total duration
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
    local marked_beats = {}
    local i = 1
    while duration > this_beat - start do
        local beat = this_beat + delay + offset
        local percent = (this_beat + offset - start) / duration
        marked_beats[i] = {
            beat = beat,
            percent = percent
        }

        this_beat = this_beat + frequency
        i = i + 1
    end
    return marked_beats
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
-- @param marked_beats - A table of marked_beat. This is expected be an array of
--                    tables each with the following fields:
--                    beat - a float of the beattime
--                    percent - a float of the percent over the duration
--                    pitch - (optional) the absolute normalized pitch value of
--                            the note. If not present, this will default to 0.0.
-- @param spawner - A function returning spawn_cmd tables. This function is
--                  expected to take a marked_beat table. It is expected to return
--                  a spawn_cmd or an array of spawn_cmds. (A spawn_cmd is
--                  considered to be an array if index 1 is not nil)
-- Note that the enemygroup will be CURR_GROUP if none is provided and the start
-- time will be the beattime given by marked_beats if none is provided

function make_actions(marked_beats, spawner)
    local i = 1
    for i, marked_beat in ipairs(marked_beats) do
        local beat = marked_beat.beat
        marked_beat["i"] = i
        local spawn_cmd = spawner(marked_beat)
        -- Single spawn_cmd
        if not spawn_cmd[1] then
            add_action_overridable(beat, CURR_GROUP, spawn_cmd)
        else
            -- Array of spawn_cmds
            for _, spawn_cmd in ipairs(spawn_cmd) do
                add_action_overridable(beat, CURR_GROUP, spawn_cmd)
            end
        end
        i = i + 1
    end
end

-- Disable the hitboxes + fades the objects of the given group. After fade_duration,
-- reenable the hitbox and disable the fade and clears the group. Note that the 
-- clear command can clear objects added during the fade_duration, so if objects
-- seem to disappear, this is why.
function fadeout_clear(time, group, fade_duration)
    local fadeout_on = {
        spawn_cmd = "set_fadeout_on",
        color = "transparent",
        duration = fade_duration
    }
    local fadeout_off = {
        spawn_cmd = "set_fadeout_off"
    }
    local hitbox_off = {
        spawn_cmd = "set_hitbox",
        value = false
    }
    local hitbox_on = {
        spawn_cmd = "set_hitbox",
        value = true
    }
    local clear_enemies = {
        spawn_cmd = "clear_enemies"
    }

    add_action(time, group, fadeout_on)
    add_action(time, group, hitbox_off)

    add_action(time + fade_duration, group, fadeout_off)
    add_action(time + fade_duration, group, hitbox_on)
    add_action(time + fade_duration, group, clear_enemies)
end

-- Return an array of angles representing sectors of a circle.
-- num_sectors - the number of sectors to make
-- num_per_sector - the number of angles per sector
-- sector_size - the size, in angles, of each sector
-- sector_gap - the gap, in angles, between each sector
function circle_sector(num_sectors, num_per_sector, sector_size, sector_gap)
    local angles = {}

    local start_angle = 0.0
    for i = 0, num_sectors do
        for j = 0, num_per_sector do
            local angle = start_angle + sector_size * (j / num_per_sector)
            table.insert(angles, angle)
        end
        start_angle = start_angle + sector_gap
    end
    return angles
end

-- Position constants
ORIGIN = pos(0.0, 0.0)
TOPLEFT = pos(-50.0, 50.0)
BOTLEFT = pos(-50.0, -50.0)
TOPRIGHT = pos(50.0, 50.0)
BOTRIGHT = pos(50.0, -50.0)

-- Midi files
buildup1main1 = add_offset(read_midi("./resources/buildup1main1.mid", 150.0), 12.0 * 4.0);
buildup1main2 = add_offset(read_midi("./resources/buildup1main2.mid", 150.0), 16.0 * 4.0);

drop1kick1 = add_offset(read_midi("./resources/drop1kick1.mid", 150.0), 20.0 * 4.0);
drop1kick2 = add_offset(read_midi("./resources/drop1kick2.mid", 150.0), 26.0 * 4.0);
drop1kick3 = add_offset(read_midi("./resources/drop1kick3.mid", 150.0), 28.0 * 4.0);
drop1kick4 = add_offset(read_midi("./resources/drop1kick4.mid", 150.0), 32.0 * 4.0);

kick1bomb = add_offset(read_midi("./resources/kick1simple.mid", 150.0), 20.0 * 4.0);

kick1solo = add_offset(read_midi("./resources/kick1solo.mid", 150.0), 20.0 * 4.0);

kick2 = add_offset(read_midi("./resources/kick2.mid", 150.0), 28.0 * 4.0);

main_melo = read_midi("./resources/mainsimpleadd.mid", 150.0);
main1 = add_offset(main_melo, 28.0 * 4.0);
main2 = add_offset(main_melo, 32.0 * 4.0);

breakkick = read_midi_grouped("./resources/break1kickgrouped.mid", 150.0);
breakkick1 = add_offset(breakkick, 36.0 * 4.0);
breaktine1 = add_offset(read_midi("./resources/break1tine1.mid", 150.0), 44.0 * 4.0);
breaktine2 = add_offset(read_midi("./resources/break1tine2.mid", 150.0), 48.0 * 4.0);
breaktine3 = add_offset(read_midi("./resources/break1tine3.mid", 150.0), 52.0 * 4.0);
breaktinesolo = add_offset(read_midi("./resources/break1tinesolo.mid", 150.0), 55.0 * 4.0);

-- Custom attacks
-- note that argument order should be: beat, percent, i, pitch
function bullet_lerp(start1, end1, start2, end2)
    return function(marked_beat)
        local t = marked_beat.percent
        local start_pos = lerp_pos(start1, start2, t)
        local end_pos = lerp_pos(end1, end2, t)
        return bullet(start_pos, end_pos)
    end
end

function bullet_player()
    return function(marked_beat)
        local i = marked_beat.i
        local the_pos;
        if i % 2 == 0 then
            the_pos = pos(-50.0, 50.0)
        else
            the_pos = pos(50.0, 50.0)
        end
        return bullet(the_pos, "player")
    end
end

function bomb_grid()
    return function()
        local grid = {
            x = math.random(-50, 50),
            y = math.random(-50, 50)
        }
        return {
            spawn_cmd = "bomb",
            pos = grid
        }
    end
end

function laser_circle(center, start_angle, end_angle)
    return function(marked_beat)
        local t = marked_beat.percent
        local angle = lerp(start_angle, end_angle, t)
        return {
            spawn_cmd = "laser",
            angle = angle,
            position = center
        }
    end
end

function laser_solo()
    return function(marked_beat)
        local side;
        if marked_beat.i % 2 == 0 then
            side = 1.0
        else
            side = -1.0
        end
        return {laser_angle(pos(50.0 * side, 0.0), 90.0), laser_angle(pos(40.0 * side, 0.0), 90.0),
                laser_angle(pos(30.0 * side, 0.0), 90.0)}
    end
end

function bullet_circle_in(center, radius, start_angle, end_angle)
    return function(marked_beat)
        local t = marked_beat.percent
        local angle = lerp(start_angle, end_angle, t)
        local start_pos = circle(center, radius, angle)
        local end_pos = center
        return bullet(start_pos, end_pos)
    end
end

function laser_diamond(clockwise)
    return function(marked_beat)
        local i = marked_beat.i
        local positions = {pos(-60.0, 0.0), pos(0.0, 60.0), pos(60.0, 0.0), pos(0.0, -60.0)}
        local step
        if clockwise then
            step = 1
        else
            step = -1
        end
        -- the weird +1 at the end is because Lua arrays are 1-indexed
        local a = positions[((step * i) % 4) + 1]
        local b = positions[((step * (i + 1)) % 4) + 1]
        return laser_points(a, b)
    end
end

function circle_sector_player_attack()
    return function(marked_beat)
        local i = marked_beat.midigroup_i
        local len = marked_beat.midigroup_len

        if i == 0 then
            local actions = {}
            local offset = math.random() * 360.0
            for j = 0, len - 1 do
                -- this assigns a unique enemygroup to each sector (specifically
                -- we get the i-index value that the sector ought to correspond
                -- to, which works since these midigroups have consecutive notes
                local enemygroup = marked_beat.i - 1 + j

                -- Set up rotations
                local sign = ternary(i % 2 == 0, 1, -1)
                local rot_on = set_rotation_on(0.0, sign * 60.0, 4.0, "player")
                local rot_off = set_rotation_off()
                rot_on["enemygroup"] = enemygroup
                rot_off["enemygroup"] = enemygroup
                rot_off["beat"] = marked_beat.beat + 4.0

                table.insert(actions, rot_on)
                table.insert(actions, rot_off)

                -- Get bullet angles
                local sector_gap
                if len == 2 then
                    sector_gap = 180.0
                elseif len == 3 then
                    sector_gap = 120.0
                else
                    sector_gap = 0.0
                end

                local offset = offset + sector_gap * j

                -- Make bullet actions
                local angles = circle_sector(1, 7, 60.0, 0.0)
                for _, angle in ipairs(angles) do

                    local pos = {
                        offset_from = circle(ORIGIN, 75.0, offset + angle)
                    }
                    local bullet = bullet(pos, "player")
                    bullet["enemygroup"] = enemygroup
                    table.insert(actions, bullet)
                end

                -- Hide the sector
                if j ~= 0 then
                    local hide = {
                        spawn_cmd = "set_render",
                        value = false,
                        enemygroup = enemygroup
                    }
                    table.insert(actions, hide)
                end

            end
            return actions
        else
            local enemygroup = marked_beat.i - 1
            return {
                spawn_cmd = "set_render",
                value = true,
                enemygroup = enemygroup
            }
        end
    end
end

function tine_attack(pitch_axis, start_coord, end_coord)
    return function(marked_beat)
        local pitch = marked_beat.pitch
        local pitch_pos = (pitch - 0.5) * 100.0
        local start_pos
        local end_pos
        if pitch_axis == "x" then
            start_pos = pos(pitch_pos, start_coord);
            end_pos = pos(pitch_pos, end_coord);
        else
            start_pos = pos(start_coord, pitch_pos);
            end_pos = pos(end_coord, pitch_pos);
        end

        return bullet(start_pos, end_pos)
    end
end

function laser_tine_attack(firing_time)
    return function(marked_beat)
        local pitch_pos = (marked_beat.pitch - 0.5) * 100.0
        local start_pos = pos(pitch_pos, 50.0)
        local end_pos = pos(-pitch_pos, -50.0)
        local laser = laser_points(start_pos, end_pos)

        local warmup = firing_time - marked_beat.beat

        laser["durations"] = durations(warmup, 1.0, 1.0)
        laser["beat"] = firing_time
        return laser
    end
end
-- Song data

-- Set up BPM, amount of song to skip, etc
table.insert(SONGMAP, {
    bpm = 150.0
})
table.insert(SONGMAP, {
    skip = 40.0 * 4.0
})

-- Measures 4 - 7 (beats 16)

make_actions(every4(4.0 * 4.0), bullet_lerp(BOTLEFT, ORIGIN, BOTRIGHT, ORIGIN))
make_actions(every4(4.0 * 4.0), bullet_lerp(TOPRIGHT, ORIGIN, TOPLEFT, ORIGIN))

-- Measures 8 - 11 (beat 32)
make_actions(every2(8.0 * 4.0), bullet_lerp(TOPLEFT, ORIGIN, BOTLEFT, ORIGIN))
make_actions(every2(8.0 * 4.0), bullet_lerp(BOTRIGHT, ORIGIN, TOPRIGHT, ORIGIN))

-- Measures 12 - 15 (beat 48)
every2offset = beat_splitter(12.0 * 4.0, 16.0, 2.0, 1.0, 0.0)
make_actions(every2(12.0 * 4.0), bullet_lerp(TOPRIGHT, TOPLEFT, BOTRIGHT, BOTLEFT))
make_actions(every2offset, bullet_lerp(BOTLEFT, BOTRIGHT, TOPLEFT, TOPRIGHT))

-- Measures 16 - 19 (beat 64)
make_actions(buildup1main2, bullet_player())

-- [DROP]
fadeout_clear(20.0 * 4.0, 0.0, 1.0)
CURR_GROUP = 1

-- Measure 24 - 27
make_actions(kick1bomb, bomb_grid())
make_actions(drop1kick1, laser_circle(ORIGIN, 0.0, -360.0 * 1.1))
make_actions(drop1kick2, laser_circle(ORIGIN, 0.0, 360.0 * 0.6))

-- Instant triple laser (beat 103)

add_action(103.0, CURR_GROUP, {
    spawn_cmd = "set_render",
    value = false
})

add_action(104.0, CURR_GROUP, {
    spawn_cmd = "set_render",
    value = true
})

CURR_GROUP = 0
make_actions(kick1solo, laser_solo());

-- Measure 28 - 35
fadeout_clear(28.0 * 4.0, 1, 1.0)

CURR_GROUP = 2
make_actions(main1, bullet_circle_in(ORIGIN, 60.0, 0.0, -360.0 * 3.1))
make_actions(main2, bullet_circle_in(ORIGIN, 60.0, 60.0, 360.0 * 3.1))

CURR_GROUP = 3
make_actions(drop1kick3, laser_diamond(true))
make_actions(drop1kick4, laser_diamond(false))

add_action(28.0 * 4.0, CURR_GROUP, set_rotation_on(0.0, 90.0, 16.0, ORIGIN))
add_action(32.0 * 4.0, CURR_GROUP, set_rotation_on(90.0, 0.0, 16.0, ORIGIN))
add_action(36.0 * 4.0, CURR_GROUP, set_rotation_off())

-- Measure 36 - 43
fadeout_clear(36.0 * 4.0, 2, 1.0)
fadeout_clear(36.0 * 4.0, 3, 1.0)
CURR_GROUP = 0

make_actions(breakkick1, circle_sector_player_attack())

-- Measure 44 - 47
make_actions(normalize_pitch(breaktine1), tine_attack("y", -50.0, 50.0))

-- Measure 48 - 51
make_actions(normalize_pitch(breaktine2), tine_attack("y", 50.0, -50.0))

-- Measure 52 - 54
make_actions(normalize_pitch(breaktine3), tine_attack("x", 50.0, -50.0))

fadeout_clear(55.0 * 4.0, CURR_GROUP, 1.0)
-- Measure 55
CURR_GROUP = 1
make_actions(normalize_pitch(breaktinesolo), laser_tine_attack(224.0))

return SONGMAP
