print("-----------\n-------------\n");

let map = default_map();
map.set_bpm(150.0);
map.set_skip_amount(0.0);

let kick1laser = parse_midi("./resources/kick1.mid", 150.0);

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
map = bullet_lerp(map, every1.with_start(16.0 * 4.0), topright, player(), topleft, player());
map = bullet_lerp(map, every1.with_start(16.0 * 4.0).with_delay(0.5), botleft, player(), botright, player());

map
