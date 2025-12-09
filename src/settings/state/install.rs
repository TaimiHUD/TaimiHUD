use {
    crate::exports::runtime as rt,
    anyhow::Context,
    core::{fmt, slice},
    rand::{Rng, RngCore, TryRngCore},
    std::{
        fs,
        io::{self, Write},
        path::Path,
    },
    uuid::Uuid,
};

pub type InstallId = Uuid;
pub type IvId = Uuid;

#[derive(Debug, Clone)]
pub struct Installation {
    pub id: InstallId,
    pub iv_id: IvId,
    pub iv: Iv,
}
impl Installation {
    pub const EMPTY: Self = Self {
        id: Self::ID_EMPTY,
        iv_id: Iv::ID_EMPTY,
        iv: Iv::EMPTY,
    };
    pub const ID_EMPTY: InstallId = Uuid::nil();

    pub fn generate_id() -> InstallId {
        Uuid::new_v4()
    }

    pub fn set_iv(&mut self, iv: Iv) {
        self.iv = iv;
        self.iv_id = self.iv.generate_id();
    }

    pub fn try_setup(&mut self) -> bool {
        let mut dirty = false;
        if self.iv_id == Iv::ID_EMPTY || self.iv.random().is_none() {
            let res = self.try_load_iv().context("loading install IV");
            if !rt::log::error_ok(res).unwrap_or(false) {
                self.set_iv(Iv::filled());
                self.save_iv();
                dirty = true;
            }
        }

        if Self::id_is_empty(&self.id) {
            self.id = Self::generate_id();
            dirty = true;
        }

        dirty
    }

    pub fn iv_path() -> &'static Path {
        Path::new("addons/Taimi/secret_id")
    }

    pub fn save_iv(&self) {
        let res = self.try_save_iv().context("recording install IV");
        let _ = rt::log::error_ok(res);
    }
    pub fn try_save_iv(&self) -> io::Result<()> {
        let Some(data) = self.iv.data() else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "IV empty?"))
        };
        let mut f = fs::File::create(Self::iv_path())?;
        f.write_all(data)
    }
    pub fn try_load_iv(&mut self) -> io::Result<bool> {
        let mut f = match fs::File::open(Self::iv_path()) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        };

        self.iv = Iv::read_from(&mut f)?;

        Ok(self.iv.random().is_some())
    }

    pub fn id_is_empty(id: &InstallId) -> bool {
        id.is_nil()
    }
}

#[derive(Clone)]
#[repr(C)]
pub struct Iv {
    /// urandom
    pub initial: [u8; Self::INITIAL_LEN],
    pub random: [u8; Self::RANDOM_LEN],
}

impl Iv {
    pub const EMPTY: Self = Self {
        initial: [0u8; Self::INITIAL_LEN],
        random: [0u8; Self::RANDOM_LEN],
    };
    pub const INITIAL_LEN: usize = 320;
    pub const RANDOM_LEN: usize = 192;
    pub const DATA_LEN: usize = Self::INITIAL_LEN + Self::RANDOM_LEN;
    pub const DATA_LEN_ID: usize = 128;

    pub fn filled() -> Self {
        Self::filled_with(&mut rand::rng(), &mut rand::rngs::OsRng)
    }
    pub fn filled_with<I: RngCore + ?Sized, R: TryRngCore + ?Sized>(
        initial_rng: &mut I,
        random_rng: &mut R,
    ) -> Self {
        let mut iv = Self::EMPTY;
        iv.fill_initial(initial_rng);
        let _ = rt::log::error_ok(iv.try_fill_random(random_rng));
        iv
    }
    pub fn fill_initial<I: RngCore + ?Sized>(&mut self, rng: &mut I) {
        rng.fill(&mut self.initial);
    }
    pub fn try_fill_random<R: TryRngCore + ?Sized>(&mut self, rng: &mut R) -> Result<(), R::Error> {
        let res = rng.try_fill_bytes(&mut self.random[..]);
        if let Err(..) = &res {
            self.random.fill(0);
        }
        res
    }

    pub fn initial(&self) -> Option<&[u8; Self::INITIAL_LEN]> {
        self.initial.iter().any(|&b| b != 0).then_some(&self.initial)
    }
    pub fn random(&self) -> Option<&[u8; Self::RANDOM_LEN]> {
        self.random.iter().any(|&b| b != 0).then_some(&self.random)
    }

    pub fn filled_len(&self) -> usize {
        let initial_len = match self.initial().is_some() {
            false => return 0,
            true => Self::INITIAL_LEN,
        };
        let random_len = self.random().is_some().then_some(Self::RANDOM_LEN).unwrap_or(0);
        initial_len + random_len
    }

    pub fn data(&self) -> Option<&[u8]> {
        match self.filled_len() {
            0 => None,
            len => Some(unsafe { slice::from_raw_parts(self.initial.as_ptr(), len) }),
        }
    }
    pub fn data_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.initial.as_mut_ptr(), Self::DATA_LEN) }
    }
    pub fn read_from<R: io::Read>(r: &mut R) -> io::Result<Self> {
        let mut iv = Self::EMPTY;
        r.read_exact(&mut iv.initial)?;
        let mut random = &mut iv.random[..];
        let mut eof = None;
        while !random.is_empty() {
            match r.read(random) {
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    eof = Some(e);
                    break
                },
                Err(e) => return Err(e),
                Ok(0) => break,
                Ok(amt) => unsafe {
                    random = random.get_unchecked_mut(amt..);
                },
            }
        }
        if random.is_empty() || random.as_ptr() == iv.random.as_ptr() {
            Ok(iv)
        } else {
            Err(eof.unwrap_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "incomplete random IV")))
        }
    }

    pub const IV_NAMESPACE: Uuid = match Uuid::try_parse_ascii(b"1f0d4b2d-3062-65aa-aa28-a46b29dc450c") {
        Ok(uuid) => uuid,
        Err(..) => unreachable!(),
    };
    pub const ID_EMPTY: IvId = Uuid::nil();
    pub fn generate_id(&self) -> IvId {
        let Some(initial) = self.initial() else { return Self::ID_EMPTY };

        let trunc = unsafe {
            const DATA_LEN_OFFSET: usize = Iv::DATA_LEN - Iv::DATA_LEN_ID;
            initial.get_unchecked(DATA_LEN_OFFSET..)
        };
        Uuid::new_v3(&Self::IV_NAMESPACE, trunc)
    }
}

impl Default for Iv {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl fmt::Debug for Iv {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Iv").field(&"<sensitive>").finish()
    }
}
