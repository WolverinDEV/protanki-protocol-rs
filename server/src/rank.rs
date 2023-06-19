macro_rules! count {
    () => (0usize);
    ( $x:tt $($xs:tt)* ) => (1usize + count!($($xs)*));
}

macro_rules! define_ranks {
    ($($name:ident { value: $value:expr, score: $score:expr, bonus: $bonus:expr })*) => {
        #[derive(Debug, Copy, Clone)]
        pub enum Rank {
            $($name, )*
        }

        static RANK_VALUES: [Rank; count!($($name)*)] = [
            $(Rank::$name,)*
        ];

        impl Rank {
            pub fn values() -> &'static [Rank] {
                &RANK_VALUES
            }

            pub fn bonus(&self) -> u32 {
                match self {
                    $(Self::$name => $bonus,)*
                }
            }

            pub fn score(&self) -> u32 {
                match self {
                    $(Self::$name => $score,)*
                }
            }

            pub fn value(&self) -> u8 {
                match self {
                    $(Self::$name => $value,)*
                }
            }
            
            pub fn from_value(value: u8) -> Option<Rank> {
                match value {
                    $($value => Some(Self::$name),)*
                    _ => None
                }
            }
        }
    };
}

impl Rank {
    pub fn next_rank(&self) -> Option<Rank> {
        let value = self.value();
        Self::from_value(value + 1)
    }

    pub fn from_score(score: u32) -> Rank {
        let mut current_rank = Rank::Recruit;
        while let Some(next_rank) = current_rank.next_rank() {
            if next_rank.score() > score {
                break;
            }

            current_rank = next_rank;
        }

        return current_rank;
    }
}

define_ranks!{
    Recruit { value: 1, score: 0, bonus: 0 }

    Private { value: 2, score: 100, bonus: 10 }
    Gefreiter { value: 3, score: 500, bonus: 40 }
    Corporal { value: 4, score: 1500, bonus: 120 }
    MasterCorporal { value: 5, score: 3700, bonus: 230 }

    Sergeant { value: 6, score: 7100, bonus: 420 }
    StaffSergeant { value: 7, score: 12300, bonus: 740 }
    MasterSergeant { value: 8, score: 20000, bonus: 950 }
    FirstSergeant { value: 9, score: 29000, bonus: 1400 }
    SergeantMajor { value: 10, score: 41000, bonus: 2000 }

    WarrantOfficer1 { value: 11, score: 57000, bonus: 2500 }
    WarrantOfficer2 { value: 12, score: 76000, bonus: 3100 }
    WarrantOfficer3 { value: 13, score: 98000, bonus: 3900 }
    WarrantOfficer4 { value: 14, score: 125000, bonus: 4600 }
    WarrantOfficer5 { value: 15, score: 156000, bonus: 5600 }

    ThirdLieutenant { value: 16, score: 192000, bonus: 6600 }
    SecondLieutenant { value: 17, score: 233000, bonus: 7900 }
    FirstLieutenant { value: 18, score: 280000, bonus: 8900 }
    Captain { value: 19, score: 332000, bonus: 10000 }

    Major { value: 20, score: 390000, bonus: 12000 }
    LieutenantColonel { value: 21, score: 455000, bonus: 14000 }
    Colonel { value: 22, score: 527000, bonus: 16000 }

    Brigadier { value: 23, score: 606000, bonus: 17000 }
    MajorGeneral { value: 24, score: 692000, bonus: 20000 }
    LieutenantGeneral { value: 25, score: 787000, bonus: 22000 }
    General { value: 26, score: 889000, bonus: 24000 }
    Marshal { value: 27, score: 1000000, bonus: 28000 }

    FieldMarshal { value: 28, score: 1122000, bonus: 31000 }
    Commander { value: 29, score: 1255000, bonus: 34000 }
    Generalissimo { value: 30, score: 1400000, bonus: 37000 }
}