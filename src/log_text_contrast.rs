// https://stackoverflow.com/questions/9733288/how-to-programmatically-calculate-the-contrast-ratio-between-two-colors

fn lum_mapper(v: f64) -> f64 {
    if v <= 0.03928 {
         return v / 12.92;
    }
    return f64::powf( (v + 0.055) / 1.055, 2.4 );
}

fn luminance(r: f64, g: f64, b: f64) -> f64 {
    let r = lum_mapper(r);
    let g = lum_mapper(g);
    let b = lum_mapper(b);

    return r * 0.2126 + g * 0.7152 + b * 0.0722;
}

fn maxf(a: f64, b: f64) -> f64 {
    if a > b {
        return a;
    }
    return b;
}

fn minf(a: f64, b: f64) -> f64 {
    if a < b {
        return a;
    }
    return b;
}

fn contrast(col1: &gdk::RGBA, col2: &gdk::RGBA) -> f64 {
    let lum1 = luminance(col1.red, col1.green, col1.blue);
    let lum2 = luminance(col2.red, col2.green, col2.blue);
    let brightest = maxf(lum1, lum2);
    let darkest = minf(lum1, lum2);
    return (brightest + 0.05) / (darkest + 0.05);
}

const RGBA_WHITE: gdk::RGBA = gdk::RGBA {
    red: 1.0,
    blue: 1.0,
    green: 1.0,
    alpha: 1.0
};

const RGBA_GREYISH: gdk::RGBA = gdk::RGBA {
    red: 0.4,
    blue: 0.4,
    green: 0.4,
    alpha: 1.0
};

const RGBA_BLACK: gdk::RGBA = gdk::RGBA {
    red: 0.0,
    blue: 0.0,
    green: 0.0,
    alpha: 1.0
};

pub fn matching_foreground_color_for_background(color: &Option<gdk::RGBA>) -> Option<gdk::RGBA> {
    match color {
        None => Some(RGBA_BLACK),
        Some(color) => {
            let c1 = contrast(color, &RGBA_WHITE);
            let c2 = contrast(color, &RGBA_GREYISH); // Add a bias toward white
            if c1 > c2 {
                return Some(RGBA_WHITE);
            }
            return Some(RGBA_BLACK);
        }
    }
}
