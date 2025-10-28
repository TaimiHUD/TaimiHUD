use {
    palette::{
        convert::{FromColorUnclamped, IntoColorUnclamped},
        rgb::Rgb,
        Srgb,
        WithAlpha,
    },
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, PartialEq, Debug, FromColorUnclamped, WithAlpha, Copy, Clone)]
#[palette(skip_derives(Rgb), rgb_standard = "palette::encoding::Srgb")]
pub struct BlishColour {
    red: u8,
    green: u8,
    blue: u8,
    #[palette(alpha)]
    alpha: f32,
}

impl BlishColour {
    pub fn imgcolor(self) -> [f32; 4] {
        let srgb: Srgb = self.into_color_unclamped();
        //let alpha = 1.0 - self.alpha;
        [srgb.red, srgb.blue, srgb.green, 1.0]
    }
}

impl<S> FromColorUnclamped<Rgb<S, f32>> for BlishColour
where
    Srgb<f32>: FromColorUnclamped<Rgb<S, f32>>,
{
    fn from_color_unclamped(color: Rgb<S, f32>) -> BlishColour {
        let srgb = Srgb::from_color_unclamped(color).into_format();

        BlishColour {
            red: srgb.red,
            green: srgb.green,
            blue: srgb.blue,
            alpha: 1.0,
        }
    }
}

impl<S> FromColorUnclamped<BlishColour> for Rgb<S, f32>
where
    Srgb<f32>: IntoColorUnclamped<Rgb<S, f32>>,
{
    fn from_color_unclamped(color: BlishColour) -> Rgb<S, f32> {
        Srgb::new(color.red, color.green, color.blue)
            .into_format()
            .into_color_unclamped()
    }
}
