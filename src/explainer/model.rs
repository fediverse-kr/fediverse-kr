//! Small, explicit examples, not an ActivityPub implementation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chapter {
    Basics,
    Delivery,
    Visibility,
    Boundaries,
    Find,
    Moving,
}

impl Chapter {
    pub const ALL: [Self; 6] = [
        Self::Basics,
        Self::Delivery,
        Self::Visibility,
        Self::Boundaries,
        Self::Find,
        Self::Moving,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::Basics => "",
            Self::Delivery => "delivery",
            Self::Visibility => "visibility",
            Self::Boundaries => "boundaries",
            Self::Find => "find",
            Self::Moving => "moving",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Basics => "한 사이트의 바깥",
            Self::Delivery => "글이 닿는 곳",
            Self::Visibility => "누구에게 보일까",
            Self::Boundaries => "연결과 거리두기",
            Self::Find => "주소로 만나기",
            Self::Moving => "다른 곳으로 이사",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Basics => 0,
            Self::Delivery => 1,
            Self::Visibility => 2,
            Self::Boundaries => 3,
            Self::Find => 4,
            Self::Moving => 5,
        }
    }
}

pub struct Beat {
    pub title: &'static str,
    pub line: &'static str,
    pub action: &'static str,
}
pub const BASICS: [Beat; 5] = [
    Beat {
        title: "평범한\nSNS 같죠.",
        line: "글을 쓰고, 팔로우하고, 답장하는 곳.",
        action: "화면 바깥으로",
    },
    Beat {
        title: "그런데 이 친구는,\n여기 회원이 아니에요.",
        line: "서로 따로 운영되는 사이트, ‘서버’가 달라요.",
        action: "주소 살펴보기",
    },
    Beat {
        title: "이름 뒤에,\n사는 곳이 붙어요.",
        line: "이메일처럼. 다른 사이트 사람도 주소로 찾아요.",
        action: "작은 집도 만나보기",
    },
    Beat {
        title: "혼자 꾸린 작은 집도\n함께 이야기해요.",
        line: "연합 기능이 있는 개인 블로그도 연결돼요.",
        action: "조금 더 넓게",
    },
    Beat {
        title: "사이트는 각자의 것.\n대화는 함께.",
        line: "이렇게 이어지는 공간들, 연합우주.",
        action: "다른 상황 살펴보기",
    },
];
pub fn beats(chapter: Chapter) -> &'static [Beat] {
    match chapter {
        Chapter::Delivery => &[
            Beat {
                title: "슈나의 서버에서,\n새 공개 글이 생겼어요.",
                line: "슈나가 글을 올리면 슈나의 서버가 새 글 활동을 만들어요.",
                action: "어디로 보내는지 보기",
            },
            Beat {
                title: "팔로워가 있는 서버로,\n새 글을 배달해요.",
                line: "페트리샤가 팔로우하므로 슈나의 서버가 냥냥.타워로 보내요.",
                action: "팔로워가 없는 서버는?",
            },
            Beat {
                title: "팔로워가 없다면,\n그 홈에는 배달하지 않아요.",
                line: "필름은 팔로우하지 않아 필름의 서버에 새 글이 배달되지 않아요.",
                action: "공개 글 주소를 열면?",
            },
            Beat {
                title: "주소로 공개 글을\n나중에 가져올 수 있어요.",
                line: "홈 배달과 공개 글을 주소로 가져오는 일은 서로 다른 경로예요.",
                action: "처음 장면으로",
            },
        ],
        Chapter::Visibility => &[
            Beat {
                title: "공개 글은,\n팔로워가 아니어도 읽어요.",
                line: "같은 글을 두 사람이 열어봅니다.",
                action: "팔로워에게만 쓴다면?",
            },
            Beat {
                title: "같은 서버여도,\n팔로워가 아니면 못 봐요.",
                line: "다른 서버의 팔로워는 볼 수 있고요.",
                action: "그럼, 조용한 공개는?",
            },
            Beat {
                title: "조용한 공개도,\n공개는 공개예요.",
                line: "공개 목록에 안 올려도, 글 주소로는 읽을 수 있어요.",
                action: "처음 장면으로",
            },
        ],
        Chapter::Find => &[
            Beat {
                title: "친구는 다른 곳에.\n검색은 내 사이트에서.",
                line: "이름만 말고, @patricia@cat.tower 전체 주소를.",
                action: "검색 결과 보기",
            },
            Beat {
                title: "여기서 누른 팔로우가,\n그곳에 도착해요.",
                line: "승인제 계정이라면, 상대가 수락할 때까지 기다려요.",
                action: "수락한 다음 보기",
            },
            Beat {
                title: "이제 내 홈에서\n새 글을 만나요.",
                line: "냥냥.타워에 새로 가입하지 않았는데도요.",
                action: "처음 장면으로",
            },
        ],
        Chapter::Moving => &[
            Beat {
                title: "사는 곳을\n바꾸고 싶다면.",
                line: "먼저 옮겨갈 곳에 새 계정을 준비해요.",
                action: "새 계정 준비 보기",
            },
            Beat {
                title: "두 계정이 모두\n내 것인지 확인하고.",
                line: "새 계정에 옛 주소를 등록한 뒤, 옛 계정에서 이사해요.",
                action: "이사 뒤의 모습 보기",
            },
            Beat {
                title: "팔로워는 새 주소로.\n옛 글은 옛집에.",
                line: "이사를 지원하는 계정의 예시예요. 모든 자료가 옮겨지지는 않아요.",
                action: "처음 장면으로",
            },
        ],
        _ => &[
            Beat {
                title: "내가 뮤트해도,\n이웃의 화면은 그대로.",
                line: "슈나만 페트리샤의 글을 안 보기로 했어요.",
                action: "차단이라면?",
            },
            Beat {
                title: "차단은,\n내 관계를 끊어요.",
                line: "서로의 팔로우는 끊기지만, 골댕의 관계는 그대로예요.",
                action: "서버 운영자가 차단한다면?",
            },
            Beat {
                title: "운영자가 연결을 끊으면,\n이웃도 영향을 받아요.",
                line: "같은 서버의 골댕도 그곳의 글을 받지 못해요.",
                action: "처음 장면으로",
            },
        ],
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Visibility {
    Public,
    Followers,
    Quiet,
}
pub fn can_read(scope: Visibility, follows: bool) -> bool {
    // This example has no mentions and assumes cooperating, connected servers.
    scope != Visibility::Followers || follows
}
#[derive(Clone, Copy, PartialEq)]
pub enum Boundary {
    Mute,
    Block,
    Suspend,
}
pub fn shows_remote_post(boundary: Boundary, is_actor: bool) -> bool {
    boundary != Boundary::Suspend && !is_actor
}
pub fn keeps_follow(boundary: Boundary, is_actor: bool) -> bool {
    boundary == Boundary::Mute || (!is_actor && boundary != Boundary::Suspend)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricted_example_is_about_relationship_not_site() {
        assert!(!can_read(Visibility::Followers, false));
        assert!(can_read(Visibility::Followers, true));
        assert!(can_read(Visibility::Quiet, false));
        assert!(can_read(Visibility::Public, false));
    }
    #[test]
    fn own_boundary_does_not_change_neighbors() {
        for mode in [Boundary::Mute, Boundary::Block] {
            assert!(!shows_remote_post(mode, true));
            assert!(shows_remote_post(mode, false));
        }
        assert!(keeps_follow(Boundary::Mute, true));
        assert!(!keeps_follow(Boundary::Block, true));
        assert!(keeps_follow(Boundary::Block, false));
    }
    #[test]
    fn suspend_scope_includes_neighbor() {
        for actor in [false, true] {
            assert!(!shows_remote_post(Boundary::Suspend, actor));
            assert!(!keeps_follow(Boundary::Suspend, actor));
        }
    }
    #[test]
    fn delivery_teaches_relationship_delivery_before_url_fetching() {
        let story = beats(Chapter::Delivery);

        assert_eq!(story.len(), 4);
        assert!(story[0].line.contains("슈나의 서버"));
        assert!(story[1].line.contains("팔로우") && story[1].line.contains("서버"));
        assert!(story[2].line.contains("필름") && story[2].line.contains("배달"));
        assert!(story[3].title.contains("주소"));
        assert!(story[3].line.contains("공개 글"));
    }
    #[test]
    fn every_followup_has_short_scroll_beats_and_a_unique_route() {
        let mut slugs = std::collections::HashSet::new();
        for chapter in Chapter::ALL {
            assert!(slugs.insert(chapter.slug()));
            if chapter == Chapter::Basics {
                continue;
            }
            assert!((3..=4).contains(&beats(chapter).len()));
            assert!(beats(chapter)
                .iter()
                .all(|b| !b.title.is_empty() && b.line.chars().count() <= 60));
        }
    }
}
