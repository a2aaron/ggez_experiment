print("-----------");

/* Some important things to keep in mind
1. Functions pass by reference! If you want to mutate an object, return it back!
Ex: do this:
    fn mutate_thing(x) {
        x.mutating_method();
        return x;
    }
    let x = mutate_thing(x);

2. If you write any function with an explicit return and put "return;" by 
   accident you will lockup the entire Rhai engine, so don't do that.
*/

let map = default_map();
map.set_bpm(150.0);
map.set_skip_amount(0.0 * 4.0);

// read midi files...
let buildup1main1 = offset_tuple(12.0 * 4.0, parse_midi("./resources/buildup1main1.mid", 150.0));
let buildup1main2 = offset_tuple(16.0 * 4.0, parse_midi("./resources/buildup1main2.mid", 150.0));

let drop1kick1 = offset_tuple(20.0 * 4.0, parse_midi("./resources/drop1kick1.mid", 150.0));
let drop1kick2 = offset_tuple(26.0 * 4.0, parse_midi("./resources/drop1kick2.mid", 150.0));
let drop1kick3 = offset_tuple(28.0 * 4.0, parse_midi("./resources/drop1kick3.mid", 150.0));
let drop1kick4 = offset_tuple(32.0 * 4.0, parse_midi("./resources/drop1kick4.mid", 150.0));

let kick1bomb = offset_tuple(20.0 * 4.0, parse_midi("./resources/kick1simple.mid", 150.0));

let kick1solo = offset_tuple(20.0 * 4.0, parse_midi("./resources/kick1solo.mid", 150.0));

let kick2 = offset_tuple(28.0 * 4.0, parse_midi("./resources/kick2.mid", 150.0));

let main_melo = parse_midi("./resources/mainsimpleadd.mid", 150.0);
let main1 = offset_tuple(28.0 * 4.0, main_melo);
let main2 = offset_tuple(32.0 * 4.0, main_melo);

let breakkick = parse_midi_grouped("./resources/break1kickgrouped.mid", 150.0);
let breakkick1 = offset_tuple_grouped(36.0 * 4.0, breakkick);
let breaktine1 = offset_tuple(44.0 * 4.0, parse_midi("./resources/break1tine1.mid", 150.0));
let breaktine2 = offset_tuple(48.0 * 4.0, parse_midi("./resources/break1tine2.mid", 150.0));
let breaktine3 = offset_tuple(52.0 * 4.0, parse_midi("./resources/break1tine3.mid", 150.0));
let breaktinesolo = offset_tuple(55.0 * 4.0, parse_midi("./resources/break1tinesolo.mid", 150.0));

let topleft = pos(-50.0, 50.0);
let topright = pos(50.0, 50.0);
let botleft = pos(-50.0, -50.0);
let botright = pos(50.0, -50.0);

// Create a beat action with the group as the current group. If beat is a MarkedBeat,
// then curr_group uses the MarkedBeat's time. If beat is an f64, then it uses
// the f64.
fn curr_group(beat, spawn_cmd) {
    let time = 0.0;
    if type_of(beat) == "f64" {
        time = beat;
    } else {
        time = get_beat(beat);
    }
    return beat_action(time, get_curr_group(), spawn_cmd);
}

// Spawn a bullet in the current enemygroup
fn bullet_curr(beat, startpos, endpos) {
    return curr_group(beat, bullet(startpos, endpos));
}

// Spawn a bomb in the current enemygroup
fn bomb_curr(beat, pos) {
    return curr_group(beat, bomb(pos));
}

// Spawn a laser thru points a and b in the current enemygroup
fn laser_curr(beat, a, b) {
    return curr_group(beat, laser(a, b, default_laser_duration()));
}

// Spawn a laser at point pos and with the given angle in the current enemygroup
fn laser_angle_curr(beat, pos, angle) {
    return curr_group(beat, laser_angle(pos, angle, default_laser_duration()));
}

// Using a beat splitter, create a set of bullets whose initial paths are along 
// start1 -> end1 and final paths along along start2 -> end2
fn bullet_lerp(map, splitter, start1, end1, start2, end2) {
    let arr = [];
    for beat_tuple in splitter {
        let beat = get_beat(beat_tuple);
        let t = get_percent(beat_tuple);

        let startpos = lerp_pos(start1, start2, t);
        let endpos = lerp_pos(end1, end2, t);
        arr.push(bullet_curr(beat, startpos, endpos));
    }
    map.add_actions(arr);
    return map;
}

// Create a set of actions using a Vec<Vec<MarkedBeat>>. Note that spawner is a closure called once
// for each midi note that takes the following arguments
// beat - the time, in beats, that the closure is called
// t    - the percentage, between 0.0 and 1.0, that beat is from the first beat in the midi
// total_i - an index, the ith time the spawner has been called at all
// this_i - an index, the ith the the spawner has been called in this midigroup
// midigroup_i - an index, the ith midigroup
// midigroup_len - the length, in notes, of this midigroup
// The spawner must return either an array of beat_actions or a single beat_action.
fn make_actions_grouped(map, beat_group_vec, spawner) {
    let arr = [];
    let total_i = 0;
    for (beat_group, midigroup_i) in beat_group_vec {
        for (beat_tuple, this_i) in beat_group {
            let beat = get_beat(beat_tuple);
            let t = get_percent(beat_tuple);
            let midigroup_len = beat_group.len();
            let beat_action = spawner.call(beat, t, total_i, this_i, midigroup_i, midigroup_len);
            
            if type_of(beat_action) == "array" {
                arr.append(beat_action)
            } else {
                arr.push(beat_action);
            }

            total_i += 1;
        }
    }
    map.add_actions(arr);
    return map;
}

// Create an action using a Vec<MarkedBeat>. Note that spawner is closure called
// with the following arguments
// beat_tuple - a MarkedBeat, containing when the beat occurs, its percentage
//              through the vector, and its normalized absolute pitch
// i          - an index, the ith time the spawner has been called
// The spawner must return either an array of beat_actions or a single beat_action.
fn make_actions(map, beat_tuple_vec, spawner) {
    let arr = [];
    for (beat_tuple, i) in beat_tuple_vec {
        let beat = get_beat(beat_tuple);
        let t = get_percent(beat_tuple);
        let pitch = get_pitch(beat_tuple);
        let beat_action = spawner.call(beat_tuple, i);
        
        if type_of(beat_action) == "array" {
            arr.append(beat_action)
        } else {
            arr.push(beat_action);
        }
    }
    map.add_actions(arr);
    return map;
}

// Spawner Closures

fn bullet_lerp(start1, end1, start2, end2) {
    return |beat, i| { 
        let startpos = lerp_pos(start1, start2, t);
        let endpos = lerp_pos(end1, end2, t);
        return bullet(beat, startpos, endpos);
    }
}

fn laser_player() {
    return |beat, i| curr_group(beat, laser(origin(), player()));
}

fn bomb_grid() {
    return |beat, i| bomb_curr(beat, grid());
}

fn laser_solo() {
    return |beat, i| {
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        return [laser_angle_curr(beat, pos(50.0 * side, 0.0), 90.0),
                laser_angle_curr(beat, pos(40.0 * side, 0.0), 90.0),
                laser_angle_curr(beat, pos(30.0 * side, 0.0), 90.0)];
    }
}

fn bullet_circle_in(center, radius, start_angle, end_angle) {
    return |beat, i| {
        let t = get_percent(beat);
        let angle = lerp(start_angle, end_angle, t);
        let startpos = circle(center.x, center.y, radius, angle);
        let endpos = center;
        return bullet_curr(beat, startpos, endpos);
    }
}

fn laser_circle(center, start_angle, end_angle) {
    return |beat, i| {
        let t = get_percent(beat);
        let angle = lerp(start_angle, end_angle, t);
        return laser_angle_curr(beat, center, angle);
    }
}

fn laser_diamond(step) {
    return |beat, i| {
        let positions = [
            pos(-60.0, 0.0),
            pos(0.0, 60.0),
            pos(60.0, 0.0),
            pos(0.0, -60.0),
        ];
        
        
        let step = if step { 1 } else { -1 };

        let a = positions[step * i % 4];
        let b = positions[step * (i + 1) % 4];

        return laser_curr(beat, a, b);
    }
}

fn bullet_player() {
    return |beat, i| {
        let pos = if i % 2 == 0 {
            pos(-50.0, 50.0)
        } else {
            pos(50.0, 50.0)
        };
        return bullet_curr(beat, pos, player());
    }
}

// Return an array of angles representing sectors of a circle.
// num_sectors - the number of sectors to make
// num_per_sector - the number of angles per sector
// sector_size - the size, in angles, of each sector
// sector_gap - the gap, in angles, between each sector
fn circle_sector(num_sectors, num_per_sector, sector_size, sector_gap) {
    let positions = [];

    let start_angle = 0.0;
    for i in range(0, num_sectors) {
        for j in range(0, num_per_sector) {
            let this_angle = start_angle + to_float(j) / to_float(num_per_sector) * sector_size; 
            positions.push(this_angle);
        }
        start_angle += sector_gap;
    }

    return positions;
}

fn circle_sector_player_attack() {
    return |beat, t, total_i, this_i, midigroup_i, midigroup_len| {
        let actions = [];
        // Spawn all the bullets at the begining of the midigroup
        // and turn each group on at the right time.
        if this_i == 0 {
            // the next midigroup_len groups will be apart of this midi group. This
            // set of actions triggers on the first note of the midigroup.
            let offset = random(0.0, 360.0);
            for group_j in range(0, midigroup_len) {
                // add 4 to avoid group collision issues w prev attack...
                let enemy_group = usize(total_i + group_j);
                // print("total_i: " + total_i + " midigroup_len: "  + midigroup_len + " enemy_group: " + (total_i + group_j));
                // set the rotation for the group...            
                let sign = if midigroup_i % 2 == 0 { 1 } else { -1 };
                let rotation = beat_action(beat, enemy_group, set_rotation_on(0.0, sign * 60.0, 4.0, player()));
                let rotation_off = beat_action(beat + 4.0, enemy_group, set_rotation_off());
                actions.push(rotation);
                actions.push(rotation_off);
                
                // get angles for the bullets
                let angles = circle_sector(1, 7, 60.0, 0.0);

                let sector_gap = 0.0;
                if midigroup_len == 2 {
                    sector_gap = 180.0;
                } else if midigroup_len == 3 {
                    sector_gap = 120.0;
                }
                let offset = offset + sector_gap * group_j;
                // now make the bullets...
                for angle in angles {
                    let angle = offset + angle;
                    let pos = circle(0.0, 0.0, 75.0, angle);
                    let pos = offset_player(pos);
                    let bullet = beat_action(beat, enemy_group, bullet(pos, player()));
                    
                    actions.push(bullet);
                }

                // and hide the sectors that correspond to not-the-first-note...
                if group_j != 0 {
                    actions.push(beat_action(beat, enemy_group, set_render(false)))
                }
            }
        } else {
            let enemy_group = usize(total_i);
            let show = set_render(true);
            let action = beat_action(beat, enemy_group, show);
            actions.push(action);
        }
        return actions;
    }
}

// Disable the hitboxes for a group and fade out the objects. Then clear the group 
// + reenable hitboxes + disable fade at time + fade_duration. Note that this 
// issues a clear command at time + fade_duration, so if you see weird disappearing
// objects, it's probably because of the clear command.
fn fadeout_clear(map, group, time, fade_duration) {
    let group = usize(group);
    let fade_color = color(1.0, 1.0, 1.0, 0.0);

    let fadeout_on = beat_action(time, group, set_fadeout_on(fade_color, fade_duration));
    let fadeout_off = beat_action(time + fade_duration, group, set_fadeout_off());
    let hitbox_off = beat_action(time, group, set_use_hitbox(false));
    let hitbox_on = beat_action(time + fade_duration, group, set_use_hitbox(true));
    let clear = beat_action(time + fade_duration, group, clear_enemies());

    map.add_action(fadeout_on);
    map.add_action(hitbox_off);

    map.add_action(fadeout_off);
    map.add_action(hitbox_on);

    map.add_action(clear);
    
    return map;
}

fn add_rotation(map, group, start_time, duration, start_angle, end_angle, rot_point) {
    let group = usize(group);
    let rotation_on = beat_action(start_time, group, set_rotation_on(start_angle, end_angle, duration, rot_point));
    let rotation_off = beat_action(start_time + duration, group, set_rotation_off());

    map.add_action(rotation_on);
    map.add_action(rotation_off);

    return map;
}

fn tine_attack(pitch_axis_is_x, start_pos, end_pos) {
    return |beat, i| {
        let pitch = get_pitch(beat);
        // let angle = pitch * 360.0;
        // let startpos = circle(0.0, 0.0, 50.0, angle);
        // let startpos = offset_player(startpos);
        let pitch_pos = (pitch - 0.5) * 100.0;
        let startpos = pos(0.0, 0.0);
        let endpos = pos(0.0, 0.0);
        if pitch_axis_is_x {
            startpos = pos(pitch_pos, start_pos);
            endpos = pos(pitch_pos, end_pos);
        } else {
            startpos = pos(start_pos, pitch_pos);
            endpos = pos(end_pos, pitch_pos);
        }

        let action = bullet_curr(beat, startpos, endpos);

        return action;
    }
}

// beatsplitter convience
let every4 = beat_splitter(0.0, 4.0);
let every2 = beat_splitter(0.0, 2.0);
let every1 = beat_splitter(0.0, 1.0);

// Actual map data

// Measures 4 - 7 (beat 16)
set_curr_group(0);

map = bullet_lerp(map, every4.with_start(4.0 * 4.0), botleft, origin(), botright, origin());
map = bullet_lerp(map, every4.with_start(4.0 * 4.0), topright, origin(), topleft, origin());


// Measures 8 - 11 (beat 32)
map = bullet_lerp(map, every2.with_start(8.0 * 4.0), topleft,  origin(), botleft,  origin());
map = bullet_lerp(map, every2.with_start(8.0 * 4.0), botright, origin(), topright, origin());

// Measures 12 - 15 (beat 48)
map = bullet_lerp(map, every2.with_start(12.0 * 4.0), topright, topleft, botright, botleft);
map = bullet_lerp(map, every2.with_start(12.0 * 4.0).with_offset(1.0), botleft, botright, topleft, topright);

// Measures 16 - 19 (beat 64)
map = make_actions(map, buildup1main2, bullet_player());
// map = bullet_lerp(map, every1.with_start(16.0 * 4.0), topright, player(), topleft, player());
// map = bullet_lerp(map, every1.with_start(16.0 * 4.0).with_delay(0.5), botleft, player(), botright, player());

// [DROP]
map = fadeout_clear(map, 0, 20.0 * 4.0, 1.0);
set_curr_group(1);

// Measure 24 - 27

map = make_actions(map, kick1bomb, bomb_grid());
map = make_actions(map, drop1kick1, laser_circle(origin(), 0.0, -360.0 * 1.1));

// Instant triple laser (beat 103)

map.add_action(curr_group(103.0, set_render_warmup(false)));
map = make_actions(map, kick1solo, laser_solo());
map.add_action(curr_group(104.0, set_render_warmup(true)));

map = make_actions(map, drop1kick2, laser_circle(origin(), 0.0, 360.0 * 0.6));

// // Measure 28 - 31
map = fadeout_clear(map, 1, 28.0 * 4.0, 1.0);
set_curr_group(2);

map = make_actions(map, main1, bullet_circle_in(origin(), 60.0, 0.0, -360.0 * 3.1));
map = make_actions(map, main2, bullet_circle_in(origin(), 60.0, 60.0, 360.0 * 3.1));

set_curr_group(3);
map = make_actions(map, drop1kick3, laser_diamond(true));
map = make_actions(map, drop1kick4, laser_diamond(false));
map.add_action(curr_group(28.0 * 4.0, set_rotation_on(0.0, 90.0, 16.0, origin())));
map.add_action(curr_group(32.0 * 4.0, set_rotation_on(90.0, 0.0, 16.0, origin())));
map.add_action(curr_group(36.0 * 4.0, set_rotation_off()));

// [BREAK]
// Measure 32 - 39
map = fadeout_clear(map, 2, 36.0 * 4.0, 1.0);
map = fadeout_clear(map, 3, 36.0 * 4.0, 1.0);

set_curr_group(0);

map = make_actions_grouped(map, breakkick1, circle_sector_player_attack());

// Measure 40 - 43

// map = add_rotation(map, 0, 40.0 * 4.0, 12.0 * 4.0, 0.0, 360.0, origin());
map = make_actions(map, normalize_pitch(breaktine1), tine_attack(false, -50.0, 50.0));
map = make_actions(map, normalize_pitch(breaktine2), tine_attack(false, 50.0, -50.0));
map = make_actions(map, normalize_pitch(breaktine3), tine_attack(true, 50.0, -50.0));

// Return the SongMap.
map
