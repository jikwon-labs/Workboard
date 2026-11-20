// Korean text for src/app/shell (see src/i18n.rs).

pub const T: &[(&str, &str)] = &[
    // top bar
    ("Go back", "뒤로 가기"),
    ("Go forward", "앞으로 가기"),
    // pinned tools (menus::TOOLS labels, drawn as tooltips)
    // rail (rail item names are keys; drawn translated)
    ("AI", "AI"),
    // sidebar
    ("Open sidebar", "사이드바 열기"),
    ("Search your sidebar", "사이드바 검색"),
    ("Filter your sidebar", "사이드바 필터"),
    ("Close sidebar", "사이드바 닫기"),
    ("Search the sidebar", "사이드바 검색"),
    ("DMs", "DM"),
    ("Today & Overdue", "오늘 및 기한 지남"),
    ("Personal List", "개인 목록"),
    ("Ask, Build, Create", "질문, 빌드, 생성"),
    ("Spaces settings", "스페이스 설정"),
    ("Create Space", "스페이스 만들기"),
    ("Expand all", "모두 펼치기"),
    ("Collapse all", "모두 접기"),
    ("All Tasks", "모든 작업"),
    ("New Space", "새 스페이스"),
    ("Create a Channel", "채널 만들기"),
    ("channel-name", "채널-이름"),
    ("Add Channel", "채널 추가"),
    // the red pill at the bottom of the sidebar when unread channels or
    // DMs are scrolled out of view below
    ("Unread messages", "안 읽은 메시지"),
    // the topic of a channel made with Add Channel
    ("Conversations about {}", "{}에 관한 대화"),
    ("\u{2014} You", "\u{2014} 나"),
    ("DM Settings", "DM 설정"),
    ("New List", "새 목록"),
    // page header
    (
        "Private: only you can see this List",
        "비공개: 나만 이 목록을 볼 수 있습니다",
    ),
    ("Favorite", "즐겨찾기"),
    ("Automate", "자동화"),
    ("Rename list", "목록 이름 바꾸기"),
    ("Edit statuses", "상태 편집"),
    ("Export to CSV", "CSV로 내보내기"),
    ("Delete list", "목록 삭제"),
    ("Start a huddle", "허들 시작"),
    ("Views", "보기"),
    // toolbar
    ("Group: {}", "그룹: {}"),
    ("Filter \u{00B7} {}", "필터 \u{00B7} {}"),
    ("Sort: {}{}", "정렬: {}{}"),
    ("Closed (tasks)", "닫힌 작업"),
    ("Show all subtasks", "모든 하위 작업 보기"),
    ("Add Task", "작업 추가"),
    ("Search tasks", "작업 검색"),
    ("Search tasks...", "작업 검색..."),
    ("Me mode: only my tasks", "나 모드: 내 작업만 보기"),
    ("Everyone", "모든 사람"),
    ("Show closed tasks", "닫힌 작업 보기"),
    ("Ascending", "오름차순"),
    ("Descending", "내림차순"),
    // list view
    ("Add a column (Custom Field)", "열 추가 (사용자 지정 필드)"),
    ("Enter a number", "숫자를 입력하세요"),
    ("All tasks here are done", "이곳의 작업을 모두 완료했습니다"),
    (
        "All {} tasks here are done",
        "이곳의 작업 {}개를 모두 완료했습니다",
    ),
    ("Show 1 closed task", "닫힌 작업 1개 보기"),
    ("Show {} closed tasks", "닫힌 작업 {}개 보기"),
    ("{} hidden", "{}개 숨김"),
    ("Closed tasks are hidden", "닫힌 작업은 숨겨져 있습니다"),
    ("No tasks here yet", "아직 작업이 없습니다"),
    // board view
    ("+  Add task", "+  작업 추가"),
    ("+  Add group", "+  그룹 추가"),
    (
        "The 1 task here is archived",
        "이곳의 작업 1개가 보관되어 있습니다",
    ),
    (
        "All {} tasks here are archived",
        "이곳의 작업 {}개가 모두 보관되어 있습니다",
    ),
    ("Show archived tasks", "보관된 작업 보기"),
    ("See archived Lists", "보관된 목록 보기"),
    (
        "Archived: hidden from the sidebar and from other views.",
        "보관됨: 사이드바와 다른 보기에서 숨겨집니다.",
    ),
    (
        "What this sits in is archived. Restore it from Settings > Spaces.",
        "이 항목이 들어 있는 곳이 보관되어 있습니다. 설정 > 스페이스에서 복원하세요.",
    ),
    ("1 archived List", "보관된 목록 1개"),
    ("{} archived Lists", "보관된 목록 {}개"),
    (
        "Restore them from Settings > Spaces",
        "설정 > 스페이스에서 복원할 수 있습니다",
    ),
];
