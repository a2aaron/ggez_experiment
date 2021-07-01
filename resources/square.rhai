print("-----------\n-------------\n");

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
map.set_skip_amount(25.0 * 4.0);

// let kick1laser = to_beat_tuple(20.0 * 4.0, parse_midi("./resources/kick1.mid", 150.0));

let buildup1main1 = to_beat_tuple(12.0 * 4.0, parse_midi("./resources/buildup1main1.mid", 150.0));
let buildup1main2 = to_beat_tuple(16.0 * 4.0, parse_midi("./resources/buildup1main2.mid", 150.0));

let drop1kick1 = to_beat_tuple(20.0 * 4.0, parse_midi("./resources/drop1kick1.mid", 150.0));
let drop1kick2 = to_beat_tuple(26.0 * 4.0, parse_midi("./resources/drop1kick2.mid", 150.0));
let drop1kick3 = to_beat_tuple(28.0 * 4.0, parse_midi("./resources/drop1kick3.mid", 150.0));
let drop1kick4 = to_beat_tuple(32.0 * 4.0, parse_midi("./resources/drop1kick4.mid", 150.0));

let kick1bomb = to_beat_tuple(20.0 * 4.0, parse_midi("./resources/kick1simple.mid", 150.0));

let kick1solo = to_beat_tuple(20.0 * 4.0, parse_midi("./resources/kick1solo.mid", 150.0));

let kick2 = to_beat_tuple(28.0 * 4.0, parse_midi("./resources/kick2.mid", 150.0));
// let main1 = to_beat_tuple(28.0 * 4.0, parse_midi("./resources/main1.mid", 150.0));

let main_melo = parse_midi("./resources/mainsimpleadd.mid", 150.0);
let main1 = to_beat_tuple(28.0 * 4.0, main_melo);
let main2 = to_beat_tuple(32.0 * 4.0, main_melo);

let topleft = pos(-50.0, 50.0);
let topright = pos(50.0, 50.0);
let botleft = pos(-50.0, -50.0);
let botright = pos(50.0, -50.0);

fn bullet_lerp(map, splitter, start1, end1, start2, end2) {
    let arr = [];
    for beat_tuple in splitter {
        let beat = get_beat(beat_tuple);
        let t = get_percent(beat_tuple);

        let startpos = lerp_pos(start1, start2, t);
        let endpos = lerp_pos(end1, end2, t);

        arr.push(bullet(beat, startpos, endpos));
    }
    map.add_actions(arr);
    return map;
}


fn make_actions(map, beat_tuple_vec, spawner) {
    let arr = [];
    for (beat_tuple, i) in beat_tuple_vec {
        let beat = get_beat(beat_tuple);
        let t = get_percent(beat_tuple);
        let beat_action = spawner.call(beat, t, i);
        
        if type_of(beat_action) == "array" {
            arr.append(beat_action)
        } else {
            arr.push(beat_action);
        }
    }
    map.add_actions(arr);
    return map;
}

fn bullet_lerp(start1, end1, start2, end2) {
    return |beat, t, i| { 
        let startpos = lerp_pos(start1, start2, t);
        let endpos = lerp_pos(end1, end2, t);
        return bullet(beat, startpos, endpos);
    }
}

fn laser_player() {
    return |beat, t, i| laser(beat, origin(), player());
}

fn bomb_grid() {
    return |beat, t, i| bomb(beat, grid());
}

fn laser_solo() {
    return |beat, t, i| {
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        return [laser_angle(beat, pos(50.0 * side, 0.0), 90.0),
                laser_angle(beat, pos(40.0 * side, 0.0), 90.0),
                laser_angle(beat, pos(30.0 * side, 0.0), 90.0)];
    }
}

fn bullet_circle_in(center, radius, start_angle, end_angle) {
    return |beat, t, i| {
        let angle = lerp(start_angle, end_angle, t);
        let startpos = circle(center.x, center.y, radius, angle);
        let endpos = center;
        return bullet(beat, startpos, endpos);
    }
}

fn laser_circle(center, start_angle, end_angle) {
    return |beat, t, i| {
        let angle = lerp(start_angle, end_angle, t);
        return laser_angle(beat, center, angle);
    }
}

fn laser_diamond(step) {
    return |beat, t, i| {
        let positions = [
            pos(-50.0, 0.0),
            pos(0.0, 50.0),
            pos(50.0, 0.0),
            pos(0.0, -50.0),
        ];
        
        
        let step = if step { 1 } else { -1 };

        let a = positions[step * i % 4];
        let b = positions[step * (i + 1) % 4];

        return laser(beat, a, b);
    }
}

fn bullet_player() {
    return |beat, t, i| {
        let pos = if i % 2 == 0 {
            pos(-50.0, 50.0)
        } else {
            pos(50.0, 50.0)
        };
        return bullet(beat, pos, player());
    }
}

fn clear_all(map, time) {
    map.add_action(section_clear(time - 0.01));
    return map;
}

let every4 = beat_splitter(0.0, 4.0);
let every2 = beat_splitter(0.0, 2.0);
let every1 = beat_splitter(0.0, 1.0);

// Measures 4 - 7 (beat 16)
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
map = clear_all(map, 20.0 * 4.0);

// Measure 24 - 27

map = make_actions(map, kick1bomb, bomb_grid());

map = make_actions(map, drop1kick1, laser_circle(origin(), 0.0, -360.0 * 1.1));

// Instant triple laser (beat 103)
// map = clear_all(map, 103.0);
map.add_action(show_warmup(103.0, false));
map = make_actions(map, kick1solo, laser_solo());
map.add_action(show_warmup(104.0, true));

map = make_actions(map, drop1kick2, laser_circle(origin(), 0.0, 360.0 * 0.6));

// Measure 28 - 31
map = clear_all(map, 28.0 * 4.0);

map = make_actions(map, main1, bullet_circle_in(origin(), 60.0, 0.0, 360.0 * 3.1));
map = make_actions(map, main2, bullet_circle_in(origin(), 60.0, 60.0, -360.0 * 3.1));

map = make_actions(map, drop1kick3, laser_diamond(true));
map = make_actions(map, drop1kick4, laser_diamond(false));

// [BREAK]
// Measure 32 - 36
map = clear_all(map, 36.0 * 4.0);

map
