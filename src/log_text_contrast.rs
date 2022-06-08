// https://stackoverflow.com/questions/9733288/how-to-programmatically-calculate-the-contrast-ratio-between-two-colors

use gtk4_helper::gtk::gdk;

fn lum_mapper(v: f32) -> f32 {
    if v <= 0.03928 {
         return v / 12.92;
    }
    return f32::powf( (v + 0.055) / 1.055, 2.4 );
}

fn luminance(r: f32, g: f32, b: f32) -> f32 {
    let r = lum_mapper(r);
    let g = lum_mapper(g);
    let b = lum_mapper(b);

    return r * 0.2126 + g * 0.7152 + b * 0.0722;
}

fn maxf(a: f32, b: f32) -> f32 {
    if a > b {
        return a;
    }
    return b;
}

fn minf(a: f32, b: f32) -> f32 {
    if a < b {
        return a;
    }
    return b;
}


fn contrast(col1: &gdk::RGBA, col2: &gdk::RGBA) -> f32 {
    let lum1 = luminance(col1.red(), col1.green(), col1.blue());
    let lum2 = luminance(col2.red(), col2.green(), col2.blue());
    let brightest = maxf(lum1, lum2);
    let darkest = minf(lum1, lum2);
    return (brightest + 0.05) / (darkest + 0.05);
}



pub fn matching_foreground_color_for_background(color: &Option<gdk::RGBA>) -> Option<gdk::RGBA> {
    let white: gdk::RGBA = gdk::RGBA::new(1.0, 1.0, 1.0, 1.0);
    let grey: gdk::RGBA = gdk::RGBA::new(0.4, 0.4, 0.4, 1.0);
    let black: gdk::RGBA = gdk::RGBA::new(0.0, 0.0, 0.0, 1.0);

    match color {
        None => Some(black),
        Some(color) => {
            let c1 = contrast(color, &white);
            let c2 = contrast(color, &grey); // Add a bias toward white
            if c1 > c2 {
                return Some(white);
            }
            return Some(black);
        }
    }
}
