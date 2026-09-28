//! Stable decorative artwork. Only hash-derived integers and trusted templates enter SVG.
use sha2::{Digest, Sha256};
const STYLE: &str = "varied-v2";
const MOTIFS: [&str; 6] = [
    "orbits",
    "landscape",
    "waves",
    "geometry",
    "constellation",
    "foliage",
];
const PALETTES: [[&str; 5]; 8] = [
    ["#E5F2F2", "#B9DCDA", "#5CA3A3", "#F2BC77", "#32646C"],
    ["#FCEDD9", "#EFD1AD", "#D28D64", "#DD6550", "#925C50"],
    ["#E9EDFA", "#CAD5F2", "#839BDD", "#F4C668", "#4D62AA"],
    ["#EFF0DB", "#DAE1B7", "#8BAB79", "#E2AF73", "#486C58"],
    ["#F4E7F0", "#E3C7DF", "#B087B4", "#F0BD9D", "#765A86"],
    ["#FFF0D8", "#F5D4A9", "#EDA76E", "#BC7294", "#956672"],
    ["#DEF0E9", "#AFDAD3", "#68AEA9", "#EAA98C", "#427783"],
    ["#FAEBE7", "#EBC8BD", "#CA7D72", "#87ADCA", "#805D76"],
];
pub fn svg_for(domain: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"fediverse.kr:header:varied-v2\0");
    hash.update(domain.as_bytes());
    let seed = hash.finalize();
    let pick =
        |i: usize, lo: i32, hi: i32| lo + i32::from(seed[i % seed.len()]) * (hi - lo + 1) / 256;
    // Modulo before narrowing keeps native and wasm32 selection byte-identical.
    let selector = u64::from_be_bytes(seed[..8].try_into().expect("SHA-256 prefix"));
    let motif = MOTIFS[(selector % MOTIFS.len() as u64) as usize];
    let palette = seed[8] as usize % PALETTES.len();
    let [paper, wash, main, accent, ink] = PALETTES[palette];
    let mut art = String::new();
    match motif {
        "orbits" => {
            let (cx, cy, radius) = (pick(9, 248, 345), pick(10, 140, 170), pick(11, 40, 57));
            let (rx, ry, angle) = (pick(12, 153, 183), pick(13, 50, 72), pick(14, -27, 27));
            art.push_str(&format!(
                "<circle cx=\"70\" cy=\"355\" r=\"{}\" fill=\"{}\"/>",
                pick(15, 140, 195),
                wash
            ));
            art.push_str(&format!(
                "<g transform=\"rotate({} {} {})\">",
                angle, cx, cy
            ));
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                cx, cy, radius, main
            ));
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"13\" fill=\"{}\"/>",
                (cx - 14),
                (cy - 17),
                wash
            ));
            art.push_str(&format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" stroke=\"{}\" stroke-width=\"3\"/>", cx, cy, rx, ry, ink));
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"12\" fill=\"{}\"/></g>",
                (cx + rx),
                cy,
                accent
            ));
            for i in 0..6usize {
                let (x, y) = (
                    pick((16 + (i as i32)) as usize, 54, 548),
                    pick((22 + (i as i32)) as usize, 55, 267),
                );
                art.push_str(&format!("<path d=\"M{} {}h8 M{} {}v8\" stroke=\"{}\" stroke-width=\"2\" stroke-linecap=\"round\"/>", (x-4), y, x, (y-4), ink));
            }
        }
        "landscape" => {
            let (peak, ridge) = (pick(9, 190, 310), pick(10, 105, 155));
            let (sunx, suny) = (pick(11, 390, 465), pick(12, 70, 103));
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                sunx,
                suny,
                pick(13, 29, 43),
                accent
            ));
            art.push_str(&format!(
                "<rect x=\"{}\" y=\"62\" width=\"92\" height=\"8\" rx=\"4\" fill=\"{}\"/>",
                pick(14, 54, 126),
                wash
            ));
            art.push_str(&format!(
                "<path d=\"M-20 220 L{} 135 L{} {} L{} 235 L650 130 V340 H-20Z\" fill=\"{}\"/>",
                (peak - 130),
                peak,
                ridge,
                (peak + 145),
                wash
            ));
            art.push_str(&format!(
                "<path d=\"M-30 260 Q95 170 210 {} Q360 110 630 212 V340 H-30Z\" fill=\"{}\"/>",
                pick(15, 201, 239),
                main
            ));
            art.push_str(&format!(
                "<path d=\"M-20 282 Q150 {} 290 280 T630 274 V340 H-20Z\" fill=\"{}\"/>",
                pick(16, 211, 258),
                ink
            ));
            art.push_str(&format!("<path d=\"M{} 185h50 M{} 198h25\" stroke=\"{}\" stroke-width=\"3\" stroke-linecap=\"round\"/>", (sunx-25), (sunx-12), paper));
        }
        "waves" => {
            let (shift, sway) = (pick(9, -22, 22), pick(10, -45, 45));
            for (i, color) in [wash, main, accent, ink].into_iter().enumerate() {
                let y = -49 + i as i32 * 102 + shift;
                art.push_str(&format!("<path d=\"M-65 {} C110 {} 345 {} 665 {}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>", y, ((y+140)+sway), ((y-128)+sway), (y+31), color, pick(((11+(i as i32))) as usize, 50, 74)));
            }
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"13\" fill=\"{}\"/>",
                pick(16, 102, 156),
                pick(17, 98, 139),
                paper
            ));
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"8\" fill=\"{}\"/>",
                pick(18, 445, 510),
                pick(19, 186, 233),
                paper
            ));
        }
        "geometry" => {
            let (angle, cx, cy) = (pick(9, -14, 14), pick(10, 294, 340), pick(11, 91, 125));
            art.push_str(&format!("<g transform=\"rotate({} 300 160)\">", angle));
            art.push_str(&format!(
                "<rect x=\"{}\" y=\"51\" width=\"139\" height=\"222\" rx=\"69\" fill=\"{}\"/>",
                pick(12, 46, 76),
                main
            ));
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                cx,
                cy,
                pick(13, 51, 65),
                accent
            ));
            art.push_str(&format!(
                "<path d=\"M{} 253 A62 62 0 0 1 {} 253Z\" fill=\"{}\"/>",
                (cx - 62),
                (cx + 62),
                ink
            ));
            art.push_str(&format!(
                "<path d=\"M403 64 H549 V263 H503 V112 H403Z\" fill=\"{}\"/>",
                wash
            ));
            art.push_str(&format!(
                "<circle cx=\"431\" cy=\"226\" r=\"27\" fill=\"{}\"/>",
                accent
            ));
            art.push_str(&format!("<path d=\"M105 173v60\" stroke=\"{}\" stroke-width=\"8\" stroke-linecap=\"round\"/>", paper));
            art.push_str("</g>");
        }
        "constellation" => {
            let points = [
                ((77 + pick(9, 0, 26)), (157 + pick(10, -40, 35))),
                ((181 + pick(11, -20, 20)), (81 + pick(12, -10, 25))),
                ((279 + pick(13, -20, 25)), (185 + pick(14, -20, 20))),
                ((381 + pick(15, -20, 20)), (106 + pick(16, -25, 25))),
                ((490 + pick(17, -15, 23)), (171 + pick(18, -30, 20))),
                ((381 + pick(19, -25, 25)), (250 + pick(20, -18, 8))),
            ];
            let path = format!(
                "M{}",
                points
                    .iter()
                    .map(|(x, y)| format!("{x} {y}"))
                    .collect::<Vec<_>>()
                    .join(" L")
            );
            art.push_str(&format!(
                "<circle cx=\"475\" cy=\"82\" r=\"{}\" fill=\"{}\"/>",
                pick(21, 72, 115),
                wash
            ));
            art.push_str(&format!(
                "<path d=\"{}\" stroke=\"{}\" stroke-width=\"3\" stroke-linejoin=\"round\"/>",
                path, main
            ));
            art.push_str(&format!(
                "<path d=\"M{} {}L{} {}\" stroke=\"{}\" stroke-width=\"2\"/>",
                points[2].0, points[2].1, points[5].0, points[5].1, main
            ));
            for (i, (x, y)) in points.iter().copied().enumerate() {
                art.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                    x,
                    y,
                    pick((22 + (i as i32)) as usize, 6, 12),
                    if ((i as i32) % 2) != 0 { accent } else { ink }
                ));
            }
            art.push_str(&format!("<path d=\"M145 242h12 M151 236v12 M519 67h12 M525 61v12\" stroke=\"{}\" stroke-width=\"2\" stroke-linecap=\"round\"/>", ink));
            art.push_str(&format!(
                "<circle cx=\"294\" cy=\"66\" r=\"3\" fill=\"{}\"/>",
                main
            ));
        }
        _ => {
            let lean = pick(9, -28, 28);
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"105\" r=\"73\" fill=\"{}\"/>",
                pick(10, 104, 172),
                wash
            ));
            art.push_str(&format!(
                "<g transform=\"translate({} 0) rotate({} 300 180)\">",
                pick(12, -34, 34),
                pick(13, -14, 14)
            ));
            art.push_str(&format!("<path d=\"M315 380 Q{} 174 {} 48 M307 270 Q179 221 130 111 M310 288 Q447 202 470 91\" stroke=\"{}\" stroke-width=\"4\" stroke-linecap=\"round\"/>", (278+lean), (315+lean), ink));
            art.push_str(&format!(
                "<path d=\"M297 220 Q197 216 199 135 Q282 141 297 220Z\" fill=\"{}\"/>",
                main
            ));
            art.push_str(&format!(
                "<path d=\"M304 158 Q339 89 401 112 Q367 179 304 158Z\" fill=\"{}\"/>",
                accent
            ));
            art.push_str(&format!(
                "<path d=\"M211 222 Q134 231 120 170 Q195 156 211 222Z\" fill=\"{}\"/>",
                accent
            ));
            art.push_str(&format!(
                "<path d=\"M180 169 Q217 108 177 66 Q126 115 180 169Z\" fill=\"{}\"/>",
                main
            ));
            art.push_str(&format!(
                "<path d=\"M398 235 Q385 155 452 155 Q487 213 398 235Z\" fill=\"{}\"/>",
                main
            ));
            art.push_str(&format!(
                "<path d=\"M443 161 Q481 144 504 178 Q480 209 443 161Z\" fill=\"{}\"/>",
                ink
            ));
            art.push_str(&format!(
                "<circle cx=\"{}\" cy=\"57\" r=\"{}\" fill=\"{}\"/>",
                (315 + lean),
                pick(11, 15, 22),
                accent
            ));
            art.push_str("</g>");
        }
    }
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"600\" height=\"320\" viewBox=\"0 0 600 320\" fill=\"none\" data-style=\"{STYLE}\" data-motif=\"{motif}\" data-palette=\"{palette}\"><rect width=\"600\" height=\"320\" fill=\"{paper}\"/>{art}</svg>\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn artwork_is_stable_bounded_diverse_and_contains_no_input_markup() {
        let mut unique = std::collections::HashSet::new();
        let mut motifs = std::collections::HashSet::new();
        let mut palettes = std::collections::HashSet::new();
        for i in 0..512 {
            let domain = format!("preview-{i}.example");
            let svg = svg_for(&domain);
            assert_eq!(svg, svg_for(&domain));
            assert!(svg.len() < 2000);
            assert!(!svg.contains(&domain));
            motifs.insert(
                svg.split("data-motif=\"")
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .to_string(),
            );
            palettes.insert(
                svg.split("data-palette=\"")
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .to_string(),
            );
            assert!(unique.insert(svg));
        }
        assert_eq!(motifs.len(), 6);
        assert_eq!(palettes.len(), 8);
        for input in [
            "예시.한국",
            "\"><script>alert(1)</script><image href=\"https://invalid.example\"/>",
            "",
        ] {
            let svg = svg_for(input);
            assert!(svg.starts_with("<svg "));
            assert!(!svg.contains("<script"));
            assert!(!svg.contains("href="));
            assert!(!svg.contains("invalid.example"));
            assert_eq!(svg, svg_for(input));
        }
    }

    #[test]
    fn matches_approved_varied_v2_goldens() {
        let vectors: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("header-art-v2-goldens.json")).unwrap();
        assert!(vectors.len() >= 16);
        for row in vectors {
            let svg = svg_for(row["domain"].as_str().unwrap());
            let actual = format!("{:x}", Sha256::digest(svg.as_bytes()));
            assert_eq!(actual, row["sha256"].as_str().unwrap());
        }
    }
}
