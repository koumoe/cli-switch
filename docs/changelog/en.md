## [0.64.2](https://github.com/koumoe/cli-switch/compare/v0.64.1...v0.64.2) (2026-08-27)

### Bug Fixes

* stabilize OpenAI quota and upstream requests (#214) ([65a5683](https://github.com/koumoe/cli-switch/commit/65a5683eeb3404e9557f433f5255ab6df3d9b1f9))
## [0.64.1](https://github.com/koumoe/cli-switch/compare/v0.64.0...v0.64.1) (2026-08-27)

### Bug Fixes

* clear OpenAI reauth flag after quota success ([037e226](https://github.com/koumoe/cli-switch/commit/037e226dd7d6bdca164b0939a7166182181c03e9))
* preserve OpenAI reauth state on transient failures ([e167d49](https://github.com/koumoe/cli-switch/commit/e167d494bfcb54528f0519b6e4929ea01b99212b))
* refine OpenAI refresh error handling ([709a04b](https://github.com/koumoe/cli-switch/commit/709a04b6ec4caaea38158b62b15e4d83202d9e2c))
* refresh OpenAI accounts without forced OAuth rotation ([3423140](https://github.com/koumoe/cli-switch/commit/3423140e76103ee0a9bcd671a96bc81a8b7cce83))
* separate account and channel names ([aa834aa](https://github.com/koumoe/cli-switch/commit/aa834aa3ce5cb1e877e2846d2b762cde214438d5))
## [0.64.0](https://github.com/koumoe/cli-switch/compare/v0.63.0...v0.64.0) (2026-08-27)

### Features

* merge remote account provider selection ([3a3dca2](https://github.com/koumoe/cli-switch/commit/3a3dca29a1f971fadbbece50dc720c3d4a411b4e))
* refine account naming and quota display ([afa4a3b](https://github.com/koumoe/cli-switch/commit/afa4a3b7341626be4eb9e2d21a428c1287ec8ad6))

### Bug Fixes

* clean up account UI semantics and tests ([2e136a9](https://github.com/koumoe/cli-switch/commit/2e136a98a3415f1eddbe5d1561ca44cb6f8441ef))
* remove unused check-in locale keys ([e332e6f](https://github.com/koumoe/cli-switch/commit/e332e6f5f2cd5232e0ee8b902b6f785b83994e5a))
## [0.63.0](https://github.com/koumoe/cli-switch/compare/v0.62.1...v0.63.0) (2026-08-26)

### Features

* add OpenAI account management UI ([3358fb7](https://github.com/koumoe/cli-switch/commit/3358fb7b660dc9224547937fda94c49b776c46ac))
* add OpenAI OAuth channels ([fe08e7a](https://github.com/koumoe/cli-switch/commit/fe08e7afb20f1f4e17214e31fdfa86e8d49dab8a))

### Bug Fixes

* preserve OpenAI channel failover ([cf5e9f7](https://github.com/koumoe/cli-switch/commit/cf5e9f77e54e4acd1137a80bb7737322918f49b5))
* remove unused account locale keys ([eb2ee66](https://github.com/koumoe/cli-switch/commit/eb2ee665f0e53de487e55a72ee36f91baac7c099))
* satisfy OpenAI CI lint ([631b523](https://github.com/koumoe/cli-switch/commit/631b5230de4aa5c8602624987455d1671a0877bf))
## [0.62.1](https://github.com/koumoe/cli-switch/compare/v0.62.0...v0.62.1) (2026-08-26)

### Bug Fixes

* complete UI account form types ([4656094](https://github.com/koumoe/cli-switch/commit/465609439aae738e5cc2638b7bb2b5e5d5cf2107))
* resolve undefined account variable in form initialization ([29b23af](https://github.com/koumoe/cli-switch/commit/29b23affb6ffd926abbd1a70b5d6a8a14b48da1b))
## [0.62.0](https://github.com/koumoe/cli-switch/compare/v0.61.1...v0.62.0) (2026-08-26)

### Features

* open account base URL from account list ([20c1f39](https://github.com/koumoe/cli-switch/commit/20c1f39544b2f57abc04089f50c8511a84a0ab5b))
* support custom account names ([2e8d71d](https://github.com/koumoe/cli-switch/commit/2e8d71dfb93da21aafa46f9879b01613f4c3d8b5))

### Bug Fixes

* persist NewAPI account names ([b812f6d](https://github.com/koumoe/cli-switch/commit/b812f6d3b0a07ef47e538efdef9054b9cb0cd2e3))
* remove unused account locale key ([3749247](https://github.com/koumoe/cli-switch/commit/3749247a685142aa6093a8f91435c3cc19b51c24))
## [0.61.1](https://github.com/koumoe/cli-switch/compare/v0.61.0...v0.61.1) (2026-08-25)

### Bug Fixes

* restore bilingual changelog bullets with issue refs ([a365b10](https://github.com/koumoe/cli-switch/commit/a365b10e864163030a90290b7f3b605d3b536604)), closes [#203](https://github.com/koumoe/cli-switch/issues/203)
* revert account names and domain links ([448345c](https://github.com/koumoe/cli-switch/commit/448345c80e104bcc1b14fb6e34d855bc797be3ca)), closes [#203](https://github.com/koumoe/cli-switch/issues/203)
* revert official Codex account OAuth ([1958ef8](https://github.com/koumoe/cli-switch/commit/1958ef81126fc33877b666387d3cffa66e84a5be)), closes [#204](https://github.com/koumoe/cli-switch/issues/204)
## [0.61.0](https://github.com/koumoe/cli-switch/compare/v0.60.0...v0.61.0) (2026-08-25)

### Features

* support official Codex account OAuth ([249823d](https://github.com/koumoe/cli-switch/commit/249823df4e147d4fd18ffee7c76b6d46f647806e))
## [0.60.0](https://github.com/koumoe/cli-switch/compare/v0.59.5...v0.60.0) (2026-08-25)

### Features

* add account names and domain links (#203) ([bb27194](https://github.com/koumoe/cli-switch/commit/bb271940f0de188597d125f512a8fa0073a55824))
## [0.59.5](https://github.com/koumoe/cli-switch/compare/v0.59.4...v0.59.5) (2026-07-16)

### Bug Fixes

* support Claude Code installation with npm 12 (#202) ([4deb848](https://github.com/koumoe/cli-switch/commit/4deb84877fb4165ee4e91092cc3d3b13303db1d2))
## [0.59.4](https://github.com/koumoe/cli-switch/compare/v0.59.3...v0.59.4) (2026-05-03)

### Bug Fixes

* break WhatsApp client event handler cycle (#201) ([e35c34e](https://github.com/koumoe/cli-switch/commit/e35c34e9ec529a2a517de34d002a7af9f2e8fc0b))
## [0.59.3](https://github.com/koumoe/cli-switch/compare/v0.59.2...v0.59.3) (2026-05-02)

### Bug Fixes

* ignore MDXEditor normalization on close (#200) ([6220a8f](https://github.com/koumoe/cli-switch/commit/6220a8f3809820aa54bdb37a692bd8747d07eb23))
## [0.59.2](https://github.com/koumoe/cli-switch/compare/v0.59.1...v0.59.2) (2026-05-02)

### Bug Fixes

* prevent WhatsApp bridge fd leak (#199) ([dcde269](https://github.com/koumoe/cli-switch/commit/dcde2697b33b87d25d527ee09bd13c40dd3e9865))
## [0.59.1](https://github.com/koumoe/cli-switch/compare/v0.59.0...v0.59.1) (2026-05-01)

### Bug Fixes

* stop whatsapp bridge restart loop (#196) ([22535be](https://github.com/koumoe/cli-switch/commit/22535bee4075011908c00918961c64f7a8f1d5d7))
## [0.59.0](https://github.com/koumoe/cli-switch/compare/v0.58.0...v0.59.0) (2026-04-16)

### Features

* refactor prompts into projects (#195) ([d247baa](https://github.com/koumoe/cli-switch/commit/d247baa401d3ac8f0ddebf5e48be7539f6a04ffc))
## [0.58.0](https://github.com/koumoe/cli-switch/compare/v0.57.3...v0.58.0) (2026-04-11)

### Features

* complete ui migration phases p0-p8 ([8bcb119](https://github.com/koumoe/cli-switch/commit/8bcb1195e470335e6e85f49ef23089a2cc00fc42))
* refine demo-aligned ui refactor ([7361389](https://github.com/koumoe/cli-switch/commit/736138995be7f5fbcf4a18bda0a66bf7407eb3d9))
* refine ui table standards and overview charts ([4d33c7f](https://github.com/koumoe/cli-switch/commit/4d33c7f478006c2d4996845e34eab6a3c75d7cf0))
* standardize remaining ui styles ([c9338be](https://github.com/koumoe/cli-switch/commit/c9338be028fd9f144e5c55e37e090d2d93df2140))

### Bug Fixes

* address ui review follow-ups ([282a933](https://github.com/koumoe/cli-switch/commit/282a933028d012f37a2bf8ecb22dd1c34560fe8b))
* keep proxy requests transparent ([4d606db](https://github.com/koumoe/cli-switch/commit/4d606db92e4818861e84facf557f2f76f37e7311))
* remove unused ui locale keys ([14be16f](https://github.com/koumoe/cli-switch/commit/14be16f0a027163274e919b4e37d3ee68d09e899))
* standardize dialog body spacing ([8d58988](https://github.com/koumoe/cli-switch/commit/8d5898861c552b894881612956d5efe79b76ae42))
## [0.57.3](https://github.com/koumoe/cli-switch/compare/v0.57.2...v0.57.3) (2026-04-05)

### Bug Fixes

* avoid redundant autostart rewrites (#192) ([0eaf996](https://github.com/koumoe/cli-switch/commit/0eaf996b95d028f9bc36504ebcb6a1efb64bb733))
## [0.57.2](https://github.com/koumoe/cli-switch/compare/v0.57.1...v0.57.2) (2026-04-04)
## [0.57.1](https://github.com/koumoe/cli-switch/compare/v0.57.0...v0.57.1) (2026-04-03)

### Bug Fixes

* reset stale sub2api auth state before relogin (#190) ([7b2d884](https://github.com/koumoe/cli-switch/commit/7b2d884da5cc9868eddf666ef431258f0865daba))
## [0.57.0](https://github.com/koumoe/cli-switch/compare/v0.56.7...v0.57.0) (2026-04-03)

### Features

* add channel retry backend ([bd088b6](https://github.com/koumoe/cli-switch/commit/bd088b63607a7ab3e188cf60b07a044cacbfaa55))
* add channel retry controls UI ([4a7348d](https://github.com/koumoe/cli-switch/commit/4a7348d9ef31c48d953270b9b2388b054ed55744))
* add remote group added notifications ([e18e793](https://github.com/koumoe/cli-switch/commit/e18e7931272eee6723c01e95d616fb0b3a8f9de9))
## [0.56.7](https://github.com/koumoe/cli-switch/compare/v0.56.6...v0.56.7) (2026-04-02)

### Bug Fixes

* normalize overview official credit prefix (#187) ([ed1f19e](https://github.com/koumoe/cli-switch/commit/ed1f19ee0d8cac9a2ba93bd6a3c17d5b93d34eef))
## [0.56.6](https://github.com/koumoe/cli-switch/compare/v0.56.5...v0.56.6) (2026-04-02)

### Bug Fixes

* apply rustfmt cleanup ([b68ce8e](https://github.com/koumoe/cli-switch/commit/b68ce8ef8793cbdb66cd1139f032d282679ebb9d))
* remove unused locale keys ([4a5c3fd](https://github.com/koumoe/cli-switch/commit/4a5c3fd5e14225f1e1677480dcb0d05c436165fc))
## [0.56.5](https://github.com/koumoe/cli-switch/compare/v0.56.4...v0.56.5) (2026-04-02)

### Bug Fixes

* add User-Agent for remote account sync ([3c6d109](https://github.com/koumoe/cli-switch/commit/3c6d109e3a11babd9da71afe6ff9c2dd0f0a818c))
## [0.56.4](https://github.com/koumoe/cli-switch/compare/v0.56.3...v0.56.4) (2026-04-02)

### Bug Fixes

* add remote delete option to managed missing prompt (#183) ([2797ee0](https://github.com/koumoe/cli-switch/commit/2797ee0c94c1ef5f0d4c39986b2b19cec92543bb))
* refresh sub2api auth session (#184) ([3307acd](https://github.com/koumoe/cli-switch/commit/3307acdfa04b44473d8805fcfb67a2564c88574b))
## [0.56.3](https://github.com/koumoe/cli-switch/compare/v0.56.2...v0.56.3) (2026-03-31)

### Bug Fixes

* move account provider badge into dedicated table column ([a13eb94](https://github.com/koumoe/cli-switch/commit/a13eb946ae14c090349fd319bc9939ae0b9bdd1d))
## [0.56.2](https://github.com/koumoe/cli-switch/compare/v0.56.1...v0.56.2) (2026-03-31)

### Bug Fixes

* refine sub2api account display and balance (#179) ([1b639b2](https://github.com/koumoe/cli-switch/commit/1b639b2f2736c7bf0d96124cd7f430081e24e28d))
## [0.56.1](https://github.com/koumoe/cli-switch/compare/v0.56.0...v0.56.1) (2026-03-31)

### Bug Fixes

* correct sub2api onboarding and auth flow ([e4cbe25](https://github.com/koumoe/cli-switch/commit/e4cbe25da09b68477998e0c8b749de75f82c5eb2))
* remove unused sub2api locale keys ([12e38a3](https://github.com/koumoe/cli-switch/commit/12e38a3f98068776432b5a07dd28350dc74a9a0c))
## [0.56.0](https://github.com/koumoe/cli-switch/compare/v0.55.0...v0.56.0) (2026-03-30)

### Features

* add remote account backend for sub2api ([66c1309](https://github.com/koumoe/cli-switch/commit/66c130999d47102aa2df99107d34f856c9318b09))
* add remote account wizard for users ([53537b0](https://github.com/koumoe/cli-switch/commit/53537b08fc88008c2b38522601c572ff5bbc96c2))
* unify remote account managed channels and maintenance ([4265db6](https://github.com/koumoe/cli-switch/commit/4265db692e4efda9695833562f5acc9af0c1ace4))
* unify remote account wizard flows ([006c3c4](https://github.com/koumoe/cli-switch/commit/006c3c44a6ea402d47cc7c0b3772069a9b247481))

### Bug Fixes

* apply rustfmt after remote account changes ([716be6b](https://github.com/koumoe/cli-switch/commit/716be6b8d7daefa33b094f5341cfca9de76d79a2))
* harden remote account provider handling ([579ee42](https://github.com/koumoe/cli-switch/commit/579ee421cf33a38bbbabc76e217316ded5c0ee5b))
* remove stale i18n keys after remote account unification ([b735ec9](https://github.com/koumoe/cli-switch/commit/b735ec95cdbf4884ea8659922ded2a1b5f671467))
## [0.55.0](https://github.com/koumoe/cli-switch/compare/v0.54.0...v0.55.0) (2026-03-29)

### Features

* split system notifications settings ([6a8fd6e](https://github.com/koumoe/cli-switch/commit/6a8fd6ea8eb5e743f8018a07cf8dcfaf7b1d5407))
## [0.54.0](https://github.com/koumoe/cli-switch/compare/v0.53.0...v0.54.0) (2026-03-29)

### Features

* add system notification switches (#175) ([27cfccd](https://github.com/koumoe/cli-switch/commit/27cfccdd9e2331685212d0221f1474539fae2301))
## [0.53.0](https://github.com/koumoe/cli-switch/compare/v0.52.0...v0.53.0) (2026-03-28)

### Features

* use native system notifications (#174) ([b4e103a](https://github.com/koumoe/cli-switch/commit/b4e103a9ff43cf8b4772ce9604fd4ccaaaa6112e))
## [0.52.0](https://github.com/koumoe/cli-switch/compare/v0.51.0...v0.52.0) (2026-03-28)

### Features

* add locale formatting, Accept-Language, and i18n lint ([2c94540](https://github.com/koumoe/cli-switch/commit/2c945402e142e71e4550f8c7e0b2f09767b971c2))

### Bug Fixes

* add managed group missing prompts ([36b6fa4](https://github.com/koumoe/cli-switch/commit/36b6fa4656a8aa6c8feeb00838febd41fdcf2be5))
## [0.51.0](https://github.com/koumoe/cli-switch/compare/v0.50.0...v0.51.0) (2026-03-28)

### Features

* add scheduler triggers for auto checkin ([482edd1](https://github.com/koumoe/cli-switch/commit/482edd1c0f8af7fc06fb6259f30d225f2ac1375b))

### Bug Fixes

* complete backend i18n issue localization ([f9dbd5e](https://github.com/koumoe/cli-switch/commit/f9dbd5e2dca23845bdf34f4f26a8e8cdea8dc155))
## [0.50.0](https://github.com/koumoe/cli-switch/compare/v0.49.1...v0.50.0) (2026-03-27)

### Features

* add account api url fallback for managed channels ([9143850](https://github.com/koumoe/cli-switch/commit/914385066f5ced7eadc25f2b0c6a60b65f80a842))

### Bug Fixes

* avoid forcing window open for multiplier sync prompts ([95b7bd8](https://github.com/koumoe/cli-switch/commit/95b7bd8dd73f0a98f93d23fc6fd6daee682a4d4a))
* compact channel priorities after deletion ([5e50bf9](https://github.com/koumoe/cli-switch/commit/5e50bf948da8761255827ea9766c9e3cbb0cd85c))
* keep channel auto-sort dialog within viewport ([80ddb40](https://github.com/koumoe/cli-switch/commit/80ddb406df2ffc789f24cbc9b5fe22c5ec43d2d3))
* require explicit managed channel protocol selection ([93e2749](https://github.com/koumoe/cli-switch/commit/93e2749232c0baceb76f3c93149a42a5df40bf45))
* satisfy clippy in channel delete handler ([1511ff5](https://github.com/koumoe/cli-switch/commit/1511ff5a94923e2c3ff5c91ec152814d2a997988))
## [0.49.1](https://github.com/koumoe/cli-switch/compare/v0.49.0...v0.49.1) (2026-03-27)

### Bug Fixes

* disable managed channel created system notification ([578306c](https://github.com/koumoe/cli-switch/commit/578306c34a64b758395fdcf04eaeb0e24ef7c468))
* require confirmation for managed channel changes ([913649b](https://github.com/koumoe/cli-switch/commit/913649b0e640e63e230d1a82347ce7530317fc91))
## [0.49.0](https://github.com/koumoe/cli-switch/compare/v0.48.1...v0.49.0) (2026-03-27)

### Features

* add New API managed channel sync notifications ([356891d](https://github.com/koumoe/cli-switch/commit/356891d8959a87699a03195d50d3d05808c7ed72))

### Bug Fixes

* align chat bridge login status badge ([1fc230a](https://github.com/koumoe/cli-switch/commit/1fc230af11bce0ab073ebe005c2dd12a042073df))
* satisfy rustfmt checks ([726cf70](https://github.com/koumoe/cli-switch/commit/726cf701e9fe6d4c03cab837b2ea160fa5045767))
## [0.48.2](https://github.com/koumoe/cli-switch/compare/v0.48.1...v0.48.2) (2026-03-27)

### Bug Fixes

* align chat bridge login status badge ([1fc230a](https://github.com/koumoe/cli-switch/commit/1fc230af11bce0ab073ebe005c2dd12a042073df))
## [0.48.1](https://github.com/koumoe/cli-switch/compare/v0.48.0...v0.48.1) (2026-03-27)

### Bug Fixes

* improve select dropdown layout and add recharge currency (#160) ([16aa012](https://github.com/koumoe/cli-switch/commit/16aa0120231b124e6925c8ac9fa920cb7ab83e1e))
## [0.48.0](https://github.com/koumoe/cli-switch/compare/v0.47.0...v0.48.0) (2026-03-26)

### Features

* add new-api account management ([1930bbc](https://github.com/koumoe/cli-switch/commit/1930bbc82b4b6bad0d69a26b931713d5c527862a))

### Bug Fixes

* address new-api account review issues ([cbe949a](https://github.com/koumoe/cli-switch/commit/cbe949adfd3fb8d37f9c6100606cb7e91a6e516b))
## [0.47.0](https://github.com/koumoe/cli-switch/compare/v0.46.1...v0.47.0) (2026-03-25)

### Features

* add configurable chat bridge turn timeout ([01ba138](https://github.com/koumoe/cli-switch/commit/01ba138c5e79e18adf160bb71b47e42522055c55))

### Bug Fixes

* hard-stop timed out chat bridge turns ([a1f1f70](https://github.com/koumoe/cli-switch/commit/a1f1f70bafd43d25dbbbde3706638b9c0b1a6198))
* move chat bridge status badge below summary ([1a6af6e](https://github.com/koumoe/cli-switch/commit/1a6af6eb4ae9d3155532fab020a765436e2d4758))
## [0.46.1](https://github.com/koumoe/cli-switch/compare/v0.46.0...v0.46.1) (2026-03-25)

### Bug Fixes

* filter release assets before publishing ([afefbdb](https://github.com/koumoe/cli-switch/commit/afefbdb480b8dc7c58c1f2decfa932df3155689f))
## [0.46.0](https://github.com/koumoe/cli-switch/compare/v0.45.3...v0.46.0) (2026-03-25)

### Features

* move chat bridge QR login into dialogs ([92866b9](https://github.com/koumoe/cli-switch/commit/92866b946a101120af00072576efbf76b6e40ed9))

### Bug Fixes

* enable whatsapp pseudo streaming ([7285196](https://github.com/koumoe/cli-switch/commit/72851963a4a96d5b19121294c82fd746a194f3fc))
## [0.45.3](https://github.com/koumoe/cli-switch/compare/v0.45.2...v0.45.3) (2026-03-25)

### Bug Fixes

* allow whatsapp self-chat binding ([58eba04](https://github.com/koumoe/cli-switch/commit/58eba04cee15791d39f1523dce8d20e9deefb7e1))
* distinguish empty session states ([6fbf145](https://github.com/koumoe/cli-switch/commit/6fbf145ac5aa6d487a47f839b31a11ee7bd6e9cf))
* improve chat bridge command layout ([4bc9730](https://github.com/koumoe/cli-switch/commit/4bc9730ada422c53b836165369f4dffc1d99a80c))
## [0.45.2](https://github.com/koumoe/cli-switch/compare/v0.45.1...v0.45.2) (2026-03-24)

### Bug Fixes

* accept WhatsApp pairing platform alias ([72ca121](https://github.com/koumoe/cli-switch/commit/72ca1211754fd57deb9e1b62fa0466ecd27539cd))
* encode Weixin login page URL in QR flow ([03a2eed](https://github.com/koumoe/cli-switch/commit/03a2eeddd6944b0791a35de793dce16b02059388))
* improve chat bridge QR and runtime error UX ([3525802](https://github.com/koumoe/cli-switch/commit/3525802332f6258f84aa50512032c43a78c7a51f))
## [0.45.1](https://github.com/koumoe/cli-switch/compare/v0.45.0...v0.45.1) (2026-03-24)

### Bug Fixes

* prevent distribution card from stretching trend chart height ([356a515](https://github.com/koumoe/cli-switch/commit/356a515967f45a609cf7bbbd0436135c1c9c975d))
* repair Weixin QR login flow in settings ([fa0bb9f](https://github.com/koumoe/cli-switch/commit/fa0bb9f585c984a3dcedaaf04917462409083d06))
## [0.45.0](https://github.com/koumoe/cli-switch/compare/v0.44.0...v0.45.0) (2026-03-24)

### Features

* improve chat help presentation (#144) ([16ea9af](https://github.com/koumoe/cli-switch/commit/16ea9afef37895c5408a8a1e3f397e0e0c44ad30))
## [0.44.0](https://github.com/koumoe/cli-switch/compare/v0.43.0...v0.44.0) (2026-03-24)

### Features

* bundle Rust WhatsApp bridge (#143) ([7550868](https://github.com/koumoe/cli-switch/commit/75508684b7941e47595b503c507beb09ffdf78f5))
## [0.43.0](https://github.com/koumoe/cli-switch/compare/v0.42.0...v0.43.0) (2026-03-24)

### Features

* add weixin chat bridge support (#142) ([e567782](https://github.com/koumoe/cli-switch/commit/e567782efb48ae1ef6faac77fdf5b76b0487d850))

### Bug Fixes

* fill overview distribution card height (#140) ([709c5df](https://github.com/koumoe/cli-switch/commit/709c5df72ab9f040b242d9d3d9638f8885bc0aa4))
## [0.42.0](https://github.com/koumoe/cli-switch/compare/v0.41.1...v0.42.0) (2026-03-23)

### Features

* inherit shell environment for CLI tools ([1f1af05](https://github.com/koumoe/cli-switch/commit/1f1af051a1108f60dfade2d93238d56d63dca8a7))

### Bug Fixes

* satisfy clippy in shell env path test ([72f32fb](https://github.com/koumoe/cli-switch/commit/72f32fb527c624bc3ab1691a7f0ea2e9971fa947))
## [0.41.1](https://github.com/koumoe/cli-switch/compare/v0.41.0...v0.41.1) (2026-03-23)

### Bug Fixes

* delay removing processing notice until final output ([98e7568](https://github.com/koumoe/cli-switch/commit/98e75682d7e9b40ffb0c2e55860167eef225f26e))
* improve telegram local file link rendering ([e35073f](https://github.com/koumoe/cli-switch/commit/e35073f4dce60e539eeb43a912a3cbfbf5187064))
* reduce overview distribution whitespace ([3f05fe4](https://github.com/koumoe/cli-switch/commit/3f05fe4cbb01659abf191195f3a670c2319492ee))
* require explicit chat bridge start flags ([c660e00](https://github.com/koumoe/cli-switch/commit/c660e00315d8f96b04fbe94690a5221ac2e1d006))
## [0.41.0](https://github.com/koumoe/cli-switch/compare/v0.40.0...v0.41.0) (2026-03-23)

### Features

* switch WhatsApp to WhatsApp Web QR login (#134) ([612989d](https://github.com/koumoe/cli-switch/commit/612989d8692fee704b2e6d089ad1cf7539e719de))
## [0.40.0](https://github.com/koumoe/cli-switch/compare/v0.39.2...v0.40.0) (2026-03-22)

### Features

* add chat bridge P3 management and WhatsApp support ([75020bb](https://github.com/koumoe/cli-switch/commit/75020bb9948d54776a1ea3960250c37e1f4c6648))

### Bug Fixes

* remove needless borrows in desktop menu text ([ca02119](https://github.com/koumoe/cli-switch/commit/ca021196946dad3ecbbf26f58e9b3752b69f4a8e))
* resolve clippy lints in status report and WhatsApp ([cdec07c](https://github.com/koumoe/cli-switch/commit/cdec07cc2126c02d08ec847389181f12b9a6390d))
## [0.39.2](https://github.com/koumoe/cli-switch/compare/v0.39.1...v0.39.2) (2026-03-22)

### Bug Fixes

* drop legacy error fallbacks in UI ([d1e0738](https://github.com/koumoe/cli-switch/commit/d1e07383b108648330ebfc530f8b0e7078c6f9c4))
* remove legacy error fields from backend ([4e43094](https://github.com/koumoe/cli-switch/commit/4e43094e304723856a4ae287a05e8bc1ff7739b9))
## [0.39.1](https://github.com/koumoe/cli-switch/compare/v0.39.0...v0.39.1) (2026-03-21)

### Bug Fixes

* clarify language setting copy ([d07f8ca](https://github.com/koumoe/cli-switch/commit/d07f8caa7a3c013e718f61d3a4480a79b6632f16))
* remove redundant /lang command ([3289593](https://github.com/koumoe/cli-switch/commit/32895938fcdb9110349e264f60dcaacf5edb4c22))
* remove stale chat binding locale state ([5b07bb4](https://github.com/koumoe/cli-switch/commit/5b07bb40ae08d88a2f7aa4f2e5e1293718d491aa))
* unify app locale for chat bridge ([41ee3fa](https://github.com/koumoe/cli-switch/commit/41ee3fa9a8287d650bb7f98ce688b609ff9584fb))
## [0.39.0](https://github.com/koumoe/cli-switch/compare/v0.38.0...v0.39.0) (2026-03-20)

### Features

* add HTTP locale context and structured error ([36ffc54](https://github.com/koumoe/cli-switch/commit/36ffc5458831dc37938a7882117963e5e2820c66))
* add preferred_locale to chat_bindings ([a03f011](https://github.com/koumoe/cli-switch/commit/a03f011085886183bf88d6386a01791b8fa3029c))
* add shared i18n foundation with AppLocale, Translator and structured issue ([898b207](https://github.com/koumoe/cli-switch/commit/898b20713cbda4ffc45438a97b6ce7749b0c7783))
* add ui_locale to app_settings ([75bb22e](https://github.com/koumoe/cli-switch/commit/75bb22e985b9470f01cfb98a58b7116008fb7477))
* frontend shared+ui two-layer locale with settings sync ([0bbfb8b](https://github.com/koumoe/cli-switch/commit/0bbfb8b3be5f4d7177623b3df1226226a86b2be4))
* migrate all chat bridge text to shared i18n ([fa26967](https://github.com/koumoe/cli-switch/commit/fa269672560d2c03e4c8ec85834febc2b24cb0db))
* use shared translator for desktop tray and menu ([cd5c7f9](https://github.com/koumoe/cli-switch/commit/cd5c7f9e52ea584e9a9d3cd2955b54cd94f958b4))

### Bug Fixes

* reduce chat bridge dispatch args for CI ([450efcb](https://github.com/koumoe/cli-switch/commit/450efcbd3eb64955304d952a314a237a28edef45))
* resolve remaining i18n regressions ([2750617](https://github.com/koumoe/cli-switch/commit/27506173fc3aae1c78495662c38feeafc34cfeee))
* satisfy clippy for update runtime ([81b1c47](https://github.com/koumoe/cli-switch/commit/81b1c47e28e7baf3517f0b10e4918483c7b58e36))
## [0.38.0](https://github.com/koumoe/cli-switch/compare/v0.37.5...v0.38.0) (2026-03-20)

### Features

* add chat bridge core runtime ([3cf31bc](https://github.com/koumoe/cli-switch/commit/3cf31bc3b6beb4187069dee38fd9ee1ebd520cc3))
* add chat bridge foundation dependencies and types ([63328cf](https://github.com/koumoe/cli-switch/commit/63328cfb913de396d52b9cb873aa633fb02aab80))
* add chat bridge settings UI ([9615b5c](https://github.com/koumoe/cli-switch/commit/9615b5c97037a5b4b204cd1d10665d720f8cd9c3))
* add chat bridge storage layer ([9e66a50](https://github.com/koumoe/cli-switch/commit/9e66a50fcb8cd6093253eafc9ea81f94f72bf586))
* add chat bridge web API and server integration ([ae64612](https://github.com/koumoe/cli-switch/commit/ae64612cf0445ab5df9ef3475b71b4fdd7c45277))
* complete chat bridge p2 platform integration ([79c1688](https://github.com/koumoe/cli-switch/commit/79c16889d23830ae27339d3c2012910f26575375))

### Bug Fixes

* chunk telegram output after formatting ([3ebc2b2](https://github.com/koumoe/cli-switch/commit/3ebc2b21b0b4cee79791061c0a044c0b7edd7860))
* dedupe attachment captions and simplify discord reconnect ([7500ca3](https://github.com/koumoe/cli-switch/commit/7500ca3a577ba05f962ecf999d13e22ad9cdadfc))
* preserve labels in telegram split messages ([f72ca51](https://github.com/koumoe/cli-switch/commit/f72ca51da95e0105f4b48faa9f8fa790d65c22fd))
* satisfy clippy for chat chunk helpers ([f3a87e6](https://github.com/koumoe/cli-switch/commit/f3a87e6d4d2dcf745b7b3cf0b428648798e2676c))
* support multi-user chat bindings per platform ([02bfd7f](https://github.com/koumoe/cli-switch/commit/02bfd7fe211b5602c12dc0988c3bc56651fb5f5d))
* tighten chat bridge message chunking ([b769fa3](https://github.com/koumoe/cli-switch/commit/b769fa36e45d54715df37c86d9c9ace1b6fcf089))
## [0.37.5](https://github.com/koumoe/cli-switch/compare/v0.37.4...v0.37.5) (2026-03-13)

### Bug Fixes

* align page layouts and improve log filters (#128) ([#128](https://github.com/koumoe/cli-switch/issues/128)) ([34a1284](https://github.com/koumoe/cli-switch/commit/34a1284904377f3271bd47cec906dd6540268171))
## [0.37.4](https://github.com/koumoe/cli-switch/compare/v0.37.3...v0.37.4) (2026-03-13)

### Bug Fixes

* reduce prompt editor build warnings ([f769b20](https://github.com/koumoe/cli-switch/commit/f769b20423769732a8bd4f4e1769fbd2387b35d3))
* upgrade rollup to resolve audit warning ([646d48e](https://github.com/koumoe/cli-switch/commit/646d48e1f38c9740b090b61d5ee79beb3e42f38e))
## [0.37.3](https://github.com/koumoe/cli-switch/compare/v0.37.2...v0.37.3) (2026-03-11)

### Bug Fixes

* align page copy and table layout ([b05ee49](https://github.com/koumoe/cli-switch/commit/b05ee498e629442412ffa3bf56cd5bc7490719f2))
* remove remaining unused locale keys ([13c5e3c](https://github.com/koumoe/cli-switch/commit/13c5e3c98712c4a5e0a456001a3203d078b3d9c9))
* remove unused locale copy ([216db5f](https://github.com/koumoe/cli-switch/commit/216db5f7e7a70c41de706e451967843847fe6376))
## [0.37.2](https://github.com/koumoe/cli-switch/compare/v0.37.1...v0.37.2) (2026-03-10)

### Bug Fixes

* align table page sizes and fixed footer layout ([44d06c5](https://github.com/koumoe/cli-switch/commit/44d06c56dc5ced8fa7bb3c8acdda8fc718708fd4))
## [0.37.1](https://github.com/koumoe/cli-switch/compare/v0.37.0...v0.37.1) (2026-03-10)

### Bug Fixes

* paginate global prompt row correctly ([2312304](https://github.com/koumoe/cli-switch/commit/2312304a0e96137ab053218a3c1d5e1e0e58c725))
## [0.37.0](https://github.com/koumoe/cli-switch/compare/v0.36.0...v0.37.0) (2026-03-10)

### Features

* improve prompts editor dialog UI and replace markdown editor (#123) ([#123](https://github.com/koumoe/cli-switch/issues/123)) ([64f2928](https://github.com/koumoe/cli-switch/commit/64f2928e3384e98e42064f00f46013bb88d37dd4))
* reduce default page size for project and monitor pages (#122) ([#122](https://github.com/koumoe/cli-switch/issues/122)) ([08e99da](https://github.com/koumoe/cli-switch/commit/08e99daf6f2dca329ef829e0aea5489b8a21535b))
## [0.36.0](https://github.com/koumoe/cli-switch/compare/v0.35.1...v0.36.0) (2026-03-09)

### Features

* improve project management workflow ([c696f18](https://github.com/koumoe/cli-switch/commit/c696f1892277d662ebadce4968a9b7e7566cea48))
* paginate monitor statistics ([c22f8eb](https://github.com/koumoe/cli-switch/commit/c22f8ebb2b3ceb3ef5533989b60f1d9cfdb40e87))

### Bug Fixes

* address review feedback ([0af9963](https://github.com/koumoe/cli-switch/commit/0af9963cb6bd82d29a41bb121c50d5552bc8b58d))
## [0.35.1](https://github.com/koumoe/cli-switch/compare/v0.35.0...v0.35.1) (2026-03-09)

### Bug Fixes

* auto-discover prompt files from cli sessions (#120) ([#120](https://github.com/koumoe/cli-switch/issues/120)) ([d24180c](https://github.com/koumoe/cli-switch/commit/d24180cbd83ca1c04271850fc9b2128799200f62))
## [0.35.0](https://github.com/koumoe/cli-switch/compare/v0.34.0...v0.35.0) (2026-03-08)

### Features

* add prompt management (#119) ([#119](https://github.com/koumoe/cli-switch/issues/119)) ([c0a5bb1](https://github.com/koumoe/cli-switch/commit/c0a5bb194cf0f270f41960d90e7738b4927d3b56))
## [0.34.0](https://github.com/koumoe/cli-switch/compare/v0.33.3...v0.34.0) (2026-02-04)

### Features

* mock Anthropic count_tokens (#118) ([#118](https://github.com/koumoe/cli-switch/issues/118)) ([b6ace9e](https://github.com/koumoe/cli-switch/commit/b6ace9e35664fdaa294bad339debac38b3e656bc))
## [0.33.3](https://github.com/koumoe/cli-switch/compare/v0.33.2...v0.33.3) (2026-02-04)

### Bug Fixes

* improve stream error handling (#117) ([#117](https://github.com/koumoe/cli-switch/issues/117)) ([5275d70](https://github.com/koumoe/cli-switch/commit/5275d70ce6d8aa3443fbdaace4a9fa2688c1395f))
## [0.33.2](https://github.com/koumoe/cli-switch/compare/v0.33.1...v0.33.2) (2026-02-03)

### Bug Fixes

* simplify CLI proxy config UI ([51cd5b9](https://github.com/koumoe/cli-switch/commit/51cd5b91630f154f404df22dd1def3c54d5ef72c))
## [0.33.1](https://github.com/koumoe/cli-switch/compare/v0.33.0...v0.33.1) (2026-02-03)

### Bug Fixes

* improve CLI settings and config (#115) ([#115](https://github.com/koumoe/cli-switch/issues/115)) ([e4f6847](https://github.com/koumoe/cli-switch/commit/e4f68477f9cdb556b9886217df3a9d411ceec7eb))
## [0.33.0](https://github.com/koumoe/cli-switch/compare/v0.32.3...v0.33.0) (2026-02-03)

### Features

* add CLI proxy config API ([06d04cb](https://github.com/koumoe/cli-switch/commit/06d04cb0a2173ebc5d44735567b39ec9a345d55b))
* add CLI proxy config banner and settings panel ([a286fad](https://github.com/koumoe/cli-switch/commit/a286fade351acf4877c0c392206302619d1448cb))
## [0.32.3](https://github.com/koumoe/cli-switch/compare/v0.32.2...v0.32.3) (2026-02-02)

### Bug Fixes

* import Cpu icon to prevent settings page crash (#113) ([#113](https://github.com/koumoe/cli-switch/issues/113)) ([ee40642](https://github.com/koumoe/cli-switch/commit/ee40642918a7e3ccb3aba3e8ca9119e26d44ee1e))
## [0.32.2](https://github.com/koumoe/cli-switch/compare/v0.32.1...v0.32.2) (2026-02-02)
## [0.32.1](https://github.com/koumoe/cli-switch/compare/v0.32.0...v0.32.1) (2026-02-02)

### Bug Fixes

* treat drop after terminal SSE as success (#111) ([#111](https://github.com/koumoe/cli-switch/issues/111)) ([6713a44](https://github.com/koumoe/cli-switch/commit/6713a44ec4f7d1c0da807e9967d6ccdbc5ae5159))
## [0.32.0](https://github.com/koumoe/cli-switch/compare/v0.31.0...v0.32.0) (2026-02-02)

### Features

* remove base deps UI and keep npm env automatic (#110) ([#110](https://github.com/koumoe/cli-switch/issues/110)) ([682ccf1](https://github.com/koumoe/cli-switch/commit/682ccf176b41578f4b8957a007477f13dbbe418e))
## [0.31.0](https://github.com/koumoe/cli-switch/compare/v0.30.1...v0.31.0) (2026-02-02)

### Features

* improve cli tools detection and lock env paths (#109) ([#109](https://github.com/koumoe/cli-switch/issues/109)) ([8a7036d](https://github.com/koumoe/cli-switch/commit/8a7036db48ee370912169a77d59aacaaf6a91cda))
## [0.30.1](https://github.com/koumoe/cli-switch/compare/v0.30.0...v0.30.1) (2026-02-01)

### Bug Fixes

* improve stream_dropped diagnostics (#108) ([#108](https://github.com/koumoe/cli-switch/issues/108)) ([dd346f8](https://github.com/koumoe/cli-switch/commit/dd346f8d3f53f1eb1318fa27649dcf1f3f7c9a87))
## [0.30.0](https://github.com/koumoe/cli-switch/compare/v0.29.1...v0.30.0) (2026-02-01)

### Features

* detect cli tool install method (#107) ([#107](https://github.com/koumoe/cli-switch/issues/107)) ([6f9cefb](https://github.com/koumoe/cli-switch/commit/6f9cefbc92fedcc5946c4f50bfe76612154746e3))
## [0.29.1](https://github.com/koumoe/cli-switch/compare/v0.29.0...v0.29.1) (2026-02-01)

### Bug Fixes

* auto npm registry selection and avoid hangs (#106) ([#106](https://github.com/koumoe/cli-switch/issues/106)) ([41f95aa](https://github.com/koumoe/cli-switch/commit/41f95aa67ae9b9c972a357c418c16abfc4458771))
## [0.29.0](https://github.com/koumoe/cli-switch/compare/v0.28.0...v0.29.0) (2026-02-01)

### Features

* allow configuring npm registry for cli tools ([2a8c2ff](https://github.com/koumoe/cli-switch/commit/2a8c2ff5359e3aef1a08786b50aee5929f0033d8))
* install cli tools into managed npm prefix ([37d4120](https://github.com/koumoe/cli-switch/commit/37d41205111c1bdc7e6558f44e2a5e0a720d6c37))
## [0.28.0](https://github.com/koumoe/cli-switch/compare/v0.27.0...v0.28.0) (2026-02-01)

### Features

* prefer system package manager for Node.js env ([9970a55](https://github.com/koumoe/cli-switch/commit/9970a55b87ae56722be10dd26c311ebdf4535bf4))
* show npm env install progress messages ([6a6c508](https://github.com/koumoe/cli-switch/commit/6a6c5085a81c44c610f9dea8eda076a43d802c9b))

### Bug Fixes

* avoid npm env install hang ([63c0235](https://github.com/koumoe/cli-switch/commit/63c0235bd45a34fb739c72d8a84d70bfd995e1de))
* localize npm env progress ([9745387](https://github.com/koumoe/cli-switch/commit/9745387e116abd9f874ca2a2cd6c33a1204c8ea3))
* resolve clippy warnings ([1d9fbcb](https://github.com/koumoe/cli-switch/commit/1d9fbcb603e72b76f80a7c72f80b61f6ed1820a7))
## [0.27.0](https://github.com/koumoe/cli-switch/compare/v0.26.0...v0.27.0) (2026-01-31)

### Features

* hot-restart backend on LAN toggle (#103) ([#103](https://github.com/koumoe/cli-switch/issues/103)) ([adc1471](https://github.com/koumoe/cli-switch/commit/adc1471530f698d754fc2fb7b0d07f26d825f1c9))
## [0.26.0](https://github.com/koumoe/cli-switch/compare/v0.25.12...v0.26.0) (2026-01-31)

### Features

* add LAN accessible toggle (#102) ([#102](https://github.com/koumoe/cli-switch/issues/102)) ([f5e934b](https://github.com/koumoe/cli-switch/commit/f5e934b9cf8a151426f6452ee270c38d6e26e0ce))
## [0.25.12](https://github.com/koumoe/cli-switch/compare/v0.25.11...v0.25.12) (2026-01-28)

### Bug Fixes

* fallback count_tokens across channels ([c978bd7](https://github.com/koumoe/cli-switch/commit/c978bd7e06859eed6e0dc0a3cd987a3deea91833))
## [0.25.11](https://github.com/koumoe/cli-switch/compare/v0.25.10...v0.25.11) (2026-01-23)

### Bug Fixes

* correct indentation in server.rs ([f93c6aa](https://github.com/koumoe/cli-switch/commit/f93c6aabf6c39988028f4c23601faf16f474c6d1))

### Performance Improvements

* cache settings/channels for proxy ([730211c](https://github.com/koumoe/cli-switch/commit/730211c83a045e304aa7bcf622819a913251749b))
## [0.25.10](https://github.com/koumoe/cli-switch/compare/v0.25.8...v0.25.10) (2026-01-23)

### Bug Fixes

* avoid clobbering shell rc on read errors ([c20bccb](https://github.com/koumoe/cli-switch/commit/c20bccbcc0319a7e70fe5df0a9be38140d4c5270))
* avoid defaulting settings during startup ([803afd8](https://github.com/koumoe/cli-switch/commit/803afd8a5ff680b531eee86ed8b7a21e0720af46))
* avoid panic when truncating GitHub error body ([51b577c](https://github.com/koumoe/cli-switch/commit/51b577c77789ab676c1d3f1df8f5ac1aa5af753f))
* bound usage event writes with a queue ([07c7062](https://github.com/koumoe/cli-switch/commit/07c70626a2447a15f5687b63a577fe73c1a606d1))
* download ui-dist into ui/dist ([70d2ace](https://github.com/koumoe/cli-switch/commit/70d2ace0db5eab58de73dc54c0df8e8d62beb224))
* enforce required channel fields ([5f635ed](https://github.com/koumoe/cli-switch/commit/5f635ed8eb7c80140f694a53dbb8047faa1925a2))
* expose app_settings schema helper ([58410e4](https://github.com/koumoe/cli-switch/commit/58410e401989ae3a4fee269b62ce4f91bd1f6e98))
* expose invalid settings flag ([c527551](https://github.com/koumoe/cli-switch/commit/c527551a3f2e8bba3387959f8369b854cca5b4f5))
* improve GitHub rate limit errors ([85bb09c](https://github.com/koumoe/cli-switch/commit/85bb09c1ad939845ccd17c6bc9c03cde3e682e97))
* make db fallbacks explicit ([e32747e](https://github.com/koumoe/cli-switch/commit/e32747e763e1fd5bb0c15c0ead5aec17a5fc35b8))
* reduce sqlite busy_timeout to 1s ([a6d10d3](https://github.com/koumoe/cli-switch/commit/a6d10d3060f66c1bda6a853273579f19ef25c243))
* set sqlite busy_timeout for all connections ([d432d74](https://github.com/koumoe/cli-switch/commit/d432d740f1b907033dbff0fdb352ed7f1b348e33))
* stop defaulting settings on load errors ([e88e324](https://github.com/koumoe/cli-switch/commit/e88e324cff2f463854c6199969da489298662f1f))
* use typed storage errors for handlers ([cdbf369](https://github.com/koumoe/cli-switch/commit/cdbf3693321a871b6e4071a9f4dbba75660f453d))
* validate sqlite identifiers in ensure_column ([9d4b822](https://github.com/koumoe/cli-switch/commit/9d4b82265b4b918b10faefac22df5b30a1239262))
* warn on invalid app settings values ([695c084](https://github.com/koumoe/cli-switch/commit/695c084a8b6f926adb0e9326ef7eb4563031b37a))

### Performance Improvements

* make auto-disable db writes non-blocking ([d1ace25](https://github.com/koumoe/cli-switch/commit/d1ace25a649d40e37ca08604ae69fd878b2c165e))
* optimize usage cost backfill ([5392d10](https://github.com/koumoe/cli-switch/commit/5392d101027e3ef1b1840a848853538c31c68102))
* prefer gemini model from uri ([f3b272b](https://github.com/koumoe/cli-switch/commit/f3b272b71cb75629e93ae459356013077f5eb28a))
* reuse parsed JSON for preview/usage ([e9dd1d0](https://github.com/koumoe/cli-switch/commit/e9dd1d03be5b9d167efbef683afd1c02065e52e0))
* reuse UI build in release workflow ([b647012](https://github.com/koumoe/cli-switch/commit/b6470120959006461f8fef174178cb5bae7ef974))
## [0.25.9](https://github.com/koumoe/cli-switch/compare/v0.25.8...v0.25.9) (2026-01-23)

### Bug Fixes

* avoid defaulting settings during startup ([803afd8](https://github.com/koumoe/cli-switch/commit/803afd8a5ff680b531eee86ed8b7a21e0720af46))
* avoid panic when truncating GitHub error body ([51b577c](https://github.com/koumoe/cli-switch/commit/51b577c77789ab676c1d3f1df8f5ac1aa5af753f))
* expose app_settings schema helper ([58410e4](https://github.com/koumoe/cli-switch/commit/58410e401989ae3a4fee269b62ce4f91bd1f6e98))
* expose invalid settings flag ([c527551](https://github.com/koumoe/cli-switch/commit/c527551a3f2e8bba3387959f8369b854cca5b4f5))
* improve GitHub rate limit errors ([85bb09c](https://github.com/koumoe/cli-switch/commit/85bb09c1ad939845ccd17c6bc9c03cde3e682e97))
* make db fallbacks explicit ([e32747e](https://github.com/koumoe/cli-switch/commit/e32747e763e1fd5bb0c15c0ead5aec17a5fc35b8))
* reduce sqlite busy_timeout to 1s ([a6d10d3](https://github.com/koumoe/cli-switch/commit/a6d10d3060f66c1bda6a853273579f19ef25c243))
* set sqlite busy_timeout for all connections ([d432d74](https://github.com/koumoe/cli-switch/commit/d432d740f1b907033dbff0fdb352ed7f1b348e33))
* stop defaulting settings on load errors ([e88e324](https://github.com/koumoe/cli-switch/commit/e88e324cff2f463854c6199969da489298662f1f))
* validate sqlite identifiers in ensure_column ([9d4b822](https://github.com/koumoe/cli-switch/commit/9d4b82265b4b918b10faefac22df5b30a1239262))
* warn on invalid app settings values ([695c084](https://github.com/koumoe/cli-switch/commit/695c084a8b6f926adb0e9326ef7eb4563031b37a))
## [0.25.8](https://github.com/koumoe/cli-switch/compare/v0.25.7...v0.25.8) (2026-01-21)

### Bug Fixes

* record usage on stream error/drop ([d7d53be](https://github.com/koumoe/cli-switch/commit/d7d53be586c1eda32b1049601e86af53980dbab7))
## [0.25.7](https://github.com/koumoe/cli-switch/compare/v0.25.6...v0.25.7) (2026-01-20)

### Bug Fixes

* avoid overriding TEMP in windows update apply script ([4b908fd](https://github.com/koumoe/cli-switch/commit/4b908fdd797b1c41aab30ae30e9bdebfe5e777fc))
## [0.25.6](https://github.com/koumoe/cli-switch/compare/v0.25.4...v0.25.6) (2026-01-20)

### Bug Fixes

* hide console window when spawning cmd/npm ([b517347](https://github.com/koumoe/cli-switch/commit/b51734792750cf30ca365b92826cb4ead03f0a9b))
* preserve Windows PATH type and broadcast env change ([548a80e](https://github.com/koumoe/cli-switch/commit/548a80e6ba21dfe23623951dd5baabf872370e5c))
* repair Windows env update compilation ([d85930b](https://github.com/koumoe/cli-switch/commit/d85930b0e701292ba9758ae5b4ec237d0c02b658))
## [0.25.5](https://github.com/koumoe/cli-switch/compare/v0.25.4...v0.25.5) (2026-01-20)

### Bug Fixes

* hide console window when spawning cmd/npm ([b517347](https://github.com/koumoe/cli-switch/commit/b51734792750cf30ca365b92826cb4ead03f0a9b))
* preserve Windows PATH type and broadcast env change ([548a80e](https://github.com/koumoe/cli-switch/commit/548a80e6ba21dfe23623951dd5baabf872370e5c))
## [0.25.4](https://github.com/koumoe/cli-switch/compare/v0.25.3...v0.25.4) (2026-01-19)

### Bug Fixes

* embed Windows exe icon ([51bf3bf](https://github.com/koumoe/cli-switch/commit/51bf3bf210a0afc80b78bc7d670390e6b54b7f32))
* set desktop window icon ([07282b3](https://github.com/koumoe/cli-switch/commit/07282b3be164b90a2ae0ca37282f9fb4ad0994a3))
## [0.25.3](https://github.com/koumoe/cli-switch/compare/v0.25.2...v0.25.3) (2026-01-19)

### Bug Fixes

* avoid passing helper args on Windows update restart ([3c075bb](https://github.com/koumoe/cli-switch/commit/3c075bba77dace4e30ac192b2d16b10a8ab31e24))
## [0.25.2](https://github.com/koumoe/cli-switch/compare/v0.25.1...v0.25.2) (2026-01-19)

### Bug Fixes

* add space in zh npm missing copy ([87b0e66](https://github.com/koumoe/cli-switch/commit/87b0e667dd163ff350e1c29a4cae29c35fc2746c))
* emit npm env install progress events ([50ba7f5](https://github.com/koumoe/cli-switch/commit/50ba7f58cd32801c0d4553ff1496f6118724ae15))
* show npm env install progress in UI ([896818f](https://github.com/koumoe/cli-switch/commit/896818f2da381661db227cd8c868e93cc7da07b8))
* update npm missing copy ([f12a0c7](https://github.com/koumoe/cli-switch/commit/f12a0c71506e041461c82bb7d10e17e80b8d5c5a))
## [0.25.1](https://github.com/koumoe/cli-switch/compare/v0.25.0...v0.25.1) (2026-01-19)

### Bug Fixes

* improve Windows update apply script (#73) ([#73](https://github.com/koumoe/cli-switch/issues/73)) ([cf5cc04](https://github.com/koumoe/cli-switch/commit/cf5cc047d18d88256250775464e68442a3005cc0))
## [0.25.0](https://github.com/koumoe/cli-switch/compare/v0.24.1...v0.25.0) (2026-01-19)

### Features

* make installed CLI tools runnable in terminal (#72) ([#72](https://github.com/koumoe/cli-switch/issues/72)) ([15a23ca](https://github.com/koumoe/cli-switch/commit/15a23ca45328285fc29f47d8eb56dd3794140b00))
## [0.24.1](https://github.com/koumoe/cli-switch/compare/v0.24.0...v0.24.1) (2026-01-18)

### Bug Fixes

* cleanup update backups and apply artifacts ([1c96bb0](https://github.com/koumoe/cli-switch/commit/1c96bb03463a8bcc8404b3f969cc980e4bae3b3e))
## [0.24.0](https://github.com/koumoe/cli-switch/compare/v0.23.1...v0.24.0) (2026-01-18)

### Features

* add base deps settings UI ([5da2432](https://github.com/koumoe/cli-switch/commit/5da2432fe18504648d332925c628740eae7916de))
* add folder picker & env validation APIs ([ca93943](https://github.com/koumoe/cli-switch/commit/ca93943d5e1ccec5e5ddc82af63478867a93370d))

### Bug Fixes

* make pick_folder build without desktop ([e2e5130](https://github.com/koumoe/cli-switch/commit/e2e51305192cd945492de9777876eb74c39cec0f))
* upgrade rfd for linux build ([f99e0cc](https://github.com/koumoe/cli-switch/commit/f99e0cc676fb3dfa133843c74ba67298321c0c5c))
## [0.23.1](https://github.com/koumoe/cli-switch/compare/v0.23.0...v0.23.1) (2026-01-15)

### Bug Fixes

* statically link MSVC CRT (#69) ([#69](https://github.com/koumoe/cli-switch/issues/69)) ([80ea1c3](https://github.com/koumoe/cli-switch/commit/80ea1c32ab4979c2d94b95b2c5e9e5d2faadf00f))
## [0.23.0](https://github.com/koumoe/cli-switch/compare/v0.22.1...v0.23.0) (2026-01-15)

### Features

* inline edit cli tools paths (#68) ([#68](https://github.com/koumoe/cli-switch/issues/68)) ([85e71a7](https://github.com/koumoe/cli-switch/commit/85e71a782057b3f54db326fa00643fd9fbb54458))
## [0.22.1](https://github.com/koumoe/cli-switch/compare/v0.21.0...v0.22.1) (2026-01-15)

### Features

* auto-install Node.js LTS for npm env ([e3d2674](https://github.com/koumoe/cli-switch/commit/e3d26741720e22dddb6a3c17d6d3f786e5af1a5d))
* prompt npm setup and keep manual paths visible ([1e88b86](https://github.com/koumoe/cli-switch/commit/1e88b86bd0d9a34f7262ce5d5f5149740fa970ab))

### Bug Fixes

* align SettingsPage indentation ([b537a6e](https://github.com/koumoe/cli-switch/commit/b537a6e43d880c418fb9d18d1a75610bef8f38d8))
* restore bilingual subjects from git log in changelog sync ([4406552](https://github.com/koumoe/cli-switch/commit/440655261ed0ed37a6e7c87806c16605156eefb6))
* satisfy clippy warnings ([dd0431a](https://github.com/koumoe/cli-switch/commit/dd0431ab6a933d6486557d109c733f0d57c0e5f6))
## [0.21.0](https://github.com/koumoe/cli-switch/compare/v0.20.0...v0.21.0) (2026-01-11)

### Features

* add UI for manual npm/node paths ([7a03705](https://github.com/koumoe/cli-switch/commit/7a037050037bcb59ddab558183f55e0bbcdc8d4f))
* allow configuring npm/node paths ([e146647](https://github.com/koumoe/cli-switch/commit/e14664780c9597a32d503fdc6b0eaac1b698d84b))

### Bug Fixes

* address clippy lints ([2e770b5](https://github.com/koumoe/cli-switch/commit/2e770b53fbf61dc364a15c42d0951f2e922e9c81))
## [0.21.0](https://github.com/koumoe/cli-switch/compare/v0.20.0...v0.21.0) (2026-01-11)

### Features

* allow configuring npm/node paths ([e146647](https://github.com/koumoe/cli-switch/commit/e146647))
* add UI for manual npm/node paths ([7a03705](https://github.com/koumoe/cli-switch/commit/7a03705))

## [0.20.0](https://github.com/koumoe/cli-switch/compare/v0.19.3...v0.20.0) (2026-01-10)

### Features

* add CLI tools status/install APIs ([7a10ade](https://github.com/koumoe/cli-switch/commit/7a10ade7b5b8014b7e1d2db4b82a9b94bfd2fccf))
* add Updates tab and CLI tools onboarding ([ca500ef](https://github.com/koumoe/cli-switch/commit/ca500ef29d64b1f9e84ddd3da69dfbdc8afbfa72))
## [0.19.3](https://github.com/koumoe/cli-switch/compare/v0.19.2...v0.19.3) (2026-01-09)

### Bug Fixes

* split bilingual changelog entries by locale ([abc78a0](https://github.com/koumoe/cli-switch/commit/abc78a0d1c53cdc357abd85f9d81615880b1a084))
## [0.19.2](https://github.com/koumoe/cli-switch/compare/v0.19.1...v0.19.2) (2026-01-09)

### Bug Fixes

* patch updater ignore + update prompt (#60) ([#60](https://github.com/koumoe/cli-switch/issues/60)) ([31f4fe6](https://github.com/koumoe/cli-switch/commit/31f4fe64ffc3d2e706e1297792200a58435c585d))
## [0.19.1](https://github.com/koumoe/cli-switch/compare/v0.19.0...v0.19.1) (2026-01-08)

### Bug Fixes

* **updater:** detach apply helper from parent session ([b6f5d2b](https://github.com/koumoe/cli-switch/commit/b6f5d2b1f24404061da3eac9866dd87c479afd38))
## [0.19.0](https://github.com/koumoe/cli-switch/compare/v0.18.3...v0.19.0) (2026-01-08)

### Features

* changelog i18n + update prompt ([#58](https://github.com/koumoe/cli-switch/issues/58)) ([18dbb2b](https://github.com/koumoe/cli-switch/commit/18dbb2bce07c5a326b2cb9b39e0d44e003edc250))
## [0.18.3](https://github.com/koumoe/cli-switch/compare/v0.18.2...v0.18.3) (2026-01-08)

### Bug Fixes

* date range picker and cleanup filters ([#57](https://github.com/koumoe/cli-switch/issues/57)) ([0fb7997](https://github.com/koumoe/cli-switch/commit/0fb79979eeb98d5a69eaf23fdff64864bbcc5611))
## [0.18.2](https://github.com/koumoe/cli-switch/compare/v0.18.1...v0.18.2) (2026-01-08)

### Bug Fixes

* improve date picker nav hit area ([16aa2e9](https://github.com/koumoe/cli-switch/commit/16aa2e983d85abef879532151e3a17f63d53064c))
## [0.18.1](https://github.com/koumoe/cli-switch/compare/v0.18.0...v0.18.1) (2026-01-08)

### Bug Fixes

* **ui:** align date picker nav buttons ([d8f9101](https://github.com/koumoe/cli-switch/commit/d8f91017ee2af706086b036af4d3fac8ffe3d916))
## [0.18.0](https://github.com/koumoe/cli-switch/compare/v0.17.0...v0.18.0) (2026-01-08)

### Features

* extend monitor time range filter ([#53](https://github.com/koumoe/cli-switch/issues/53)) ([cf0a442](https://github.com/koumoe/cli-switch/commit/cf0a4423bef1c052acd9b528e20b11af2a6e9c5f))
## [0.17.0](https://github.com/koumoe/cli-switch/compare/v0.16.0...v0.17.0) (2026-01-07)

### Features

* **api:** return error code and english message ([3ff16f4](https://github.com/koumoe/cli-switch/commit/3ff16f4b7f25dc73007aeda93de991770a95d7e7))
* **ui:** localize api error codes ([9aa1fc4](https://github.com/koumoe/cli-switch/commit/9aa1fc487f773bfcb89820de31b14c603287e1bf))
## [0.16.0](https://github.com/koumoe/cli-switch/compare/v0.15.2...v0.16.0) (2026-01-07)

### Features

* add channel check-in schema ([a1133ac](https://github.com/koumoe/cli-switch/commit/a1133ac98e7422bf690ed8652f1d0432957b17b4))
* add check-in APIs ([145a048](https://github.com/koumoe/cli-switch/commit/145a0488c5bf8e6058c977ebc50f3cd2f4a8bf38))
* **ui:** add check-in action ([a5127af](https://github.com/koumoe/cli-switch/commit/a5127af52965cb6a2682d55ea8b9b553f249472e))

### Bug Fixes

* **ui:** improve channel modal and check-in display ([ff17c3a](https://github.com/koumoe/cli-switch/commit/ff17c3ae6a93760a281f9e5e23875e5f62ba1d07))
* **ui:** rename check-in table header key ([6c2635b](https://github.com/koumoe/cli-switch/commit/6c2635be322f0b6de5904bff85ba8b1de5f6105b))
## [0.15.2](https://github.com/koumoe/cli-switch/compare/v0.15.1...v0.15.2) (2025-12-30)

### Bug Fixes

* keep channel_failures out of records clear ([49ea89f](https://github.com/koumoe/cli-switch/commit/49ea89f18bdf137d49dad7c42aec8c6f0b926c46))
* **ui:** clarify records clear toast ([57caf22](https://github.com/koumoe/cli-switch/commit/57caf2245c1f38256b93561cc98a3a102a25f4eb))
## [0.15.1](https://github.com/koumoe/cli-switch/compare/v0.15.0...v0.15.1) (2025-12-30)

### Bug Fixes

* replace stale pending update with latest ([0eca2ed](https://github.com/koumoe/cli-switch/commit/0eca2ed5a901af29611505f0933eacdaec31bf47))
* satisfy clippy in updater pending check ([2ef225d](https://github.com/koumoe/cli-switch/commit/2ef225da0685305c739b5a0f015656d59d39634d))
* **ui:** allow downloading latest when update is pending ([2630ab3](https://github.com/koumoe/cli-switch/commit/2630ab310da4919d2eb5447ee077299fda32863a))
## [0.15.0](https://github.com/koumoe/cli-switch/compare/v0.14.0...v0.15.0) (2025-12-30)

### Features

* **ui:** move toast to top-center ([d4cdbcf](https://github.com/koumoe/cli-switch/commit/d4cdbcf2bfeed4b2238580dd0a7f2aa46bd4a24e))
* **vite:** split vendor chunks ([5694e7d](https://github.com/koumoe/cli-switch/commit/5694e7dca2154e229b575a1dce1b250b57172635))
## [0.14.0](https://github.com/koumoe/cli-switch/compare/v0.13.2...v0.14.0) (2025-12-30)

### Features

* **ui:** show real multiplier in channels table ([a2e1ea6](https://github.com/koumoe/cli-switch/commit/a2e1ea625c04739bc025ff0411114f458030f4c5))
## [0.13.2](https://github.com/koumoe/cli-switch/compare/v0.13.1...v0.13.2) (2025-12-30)

### Bug Fixes

* **ui:** keep stable order for equal multipliers ([d619e92](https://github.com/koumoe/cli-switch/commit/d619e929e801982837a0db8b77fb03aac4d5177b))
## [0.13.1](https://github.com/koumoe/cli-switch/compare/v0.13.0...v0.13.1) (2025-12-24)

### Bug Fixes

* revert multi-endpoint/multi-key support ([bc21bce](https://github.com/koumoe/cli-switch/commit/bc21bced6a0494bae206c845a324ba05c9b8e39c))
## [0.13.0](https://github.com/koumoe/cli-switch/compare/v0.12.0...v0.13.0) (2025-12-24)

### Features

* **core:** multi-endpoint and multi-key with auto-disable ([695bbd7](https://github.com/koumoe/cli-switch/commit/695bbd7cc002b577f4dd8708acbf6c6269860e49))
* **settings:** add errors-only record cleanup ([5144e2c](https://github.com/koumoe/cli-switch/commit/5144e2ce5293830bd9314846084eccaba3f237e1))
* **ui:** configure multi endpoints/keys and show cooldown ([d439be3](https://github.com/koumoe/cli-switch/commit/d439be3e0e8b924c4d9738f4b9b9bd92af9c16bc))
## [0.12.0](https://github.com/koumoe/cli-switch/compare/v0.11.1...v0.12.0) (2025-12-23)

### Features

* **update:** retain last two update artifacts ([5294f08](https://github.com/koumoe/cli-switch/commit/5294f08f2e22b936ee1b4c4b6a2913aac7fcc7d1))

### Bug Fixes

* **ui:** reopen update-ready dialog on manual check ([1cf2245](https://github.com/koumoe/cli-switch/commit/1cf2245911f5c816f7bf39469d4b26aea560f4ca))
## [0.11.1](https://github.com/koumoe/cli-switch/compare/v0.11.0...v0.11.1) (2025-12-23)

### Bug Fixes

* repair logs locale key override ([40f76af](https://github.com/koumoe/cli-switch/commit/40f76afb55275220a09f0073fe678c835f530b5e))
* simplify recharge settings and validate real multiplier ([83dc8a1](https://github.com/koumoe/cli-switch/commit/83dc8a12d1f59a42c89f79e75faf29b56fefe88a))
## [0.11.0](https://github.com/koumoe/cli-switch/compare/v0.10.1...v0.11.0) (2025-12-23)

### Features

* **storage:** add recharge currency for channels ([57641c0](https://github.com/koumoe/cli-switch/commit/57641c0e840b85da07baf9fd6f2ad34bf04cf5f6))

### Bug Fixes

* **ui:** adjust cost labels and channel recharge currency ([052948c](https://github.com/koumoe/cli-switch/commit/052948c337f35c763ae74e5b546d67661cfad116))
## [0.10.1](https://github.com/koumoe/cli-switch/compare/v0.10.0...v0.10.1) (2025-12-23)
## [0.10.0](https://github.com/koumoe/cli-switch/compare/v0.9.0...v0.10.0) (2025-12-23)

### Features

* **channels:** add multipliers and auto-sort preview ([f449688](https://github.com/koumoe/cli-switch/commit/f44968808ce18f1939ab78e390a98761a268ef4c))
* **i18n:** update currency and spend labels ([558079d](https://github.com/koumoe/cli-switch/commit/558079d5b5076b95b540b81dd163b33d6e1b137d))
* **settings:** add currency display mode ([176d962](https://github.com/koumoe/cli-switch/commit/176d962c1acb58d7583da990b33f6a9ebdf7bf29))
* **ui:** distinguish estimated cost and actual spend ([a82d12e](https://github.com/koumoe/cli-switch/commit/a82d12e7e6ca5fccb676696790f8bd4190985f4c))

### Bug Fixes

* apply rustfmt ([87d9418](https://github.com/koumoe/cli-switch/commit/87d94185520c270c4e95a54d95b03a0eaaafbc1c))
## [0.9.0](https://github.com/koumoe/cli-switch/compare/v0.8.0...v0.9.0) (2025-12-22)

### Features

* **logging:** add log retention days and cleanup ([ae32d75](https://github.com/koumoe/cli-switch/commit/ae32d753079dc5127fdab6c3b827f00cca954a7b))
* **ui:** add maintenance subpage and log retention setting ([22ffa2a](https://github.com/koumoe/cli-switch/commit/22ffa2a8cb10cf53e657ccfd9a55a82122a62d1e))

### Bug Fixes

* **i18n:** update settings texts ([053d87b](https://github.com/koumoe/cli-switch/commit/053d87b20d68a7460bc2b6a249b48e021b34ff57))
* **logging:** remove dead branch in retention cleanup ([816a385](https://github.com/koumoe/cli-switch/commit/816a38528ee35a76bdc7896eba5ed00c9aedc3b2))
## [0.8.0](https://github.com/koumoe/cli-switch/compare/v0.7.0...v0.8.0) (2025-12-22)

### Features

* **ui:** refactor settings page with tabs layout ([36f3dcc](https://github.com/koumoe/cli-switch/commit/36f3dcc3d3eacac3f4dd2d72d23184293ba302a4))
## [0.7.0](https://github.com/koumoe/cli-switch/compare/v0.6.0...v0.7.0) (2025-12-22)

### Features

* **autostart:** launch minimized to tray ([a2127e6](https://github.com/koumoe/cli-switch/commit/a2127e68ddca54f917edf8aaaf3f60b0b9c094dd))
* **settings:** add autostart launch mode ([528d832](https://github.com/koumoe/cli-switch/commit/528d83247b4c5b7f7ed514f0c99d3782b76fa50f))

### Bug Fixes

* **ci:** avoid unused autostart flag ([2873c01](https://github.com/koumoe/cli-switch/commit/2873c01e8507db3a865e0b4aec3a83418af43cc9))
## [0.6.0](https://github.com/koumoe/cli-switch/compare/v0.5.0...v0.6.0) (2025-12-21)

### Features

* IPC-driven UI updates ([#31](https://github.com/koumoe/cli-switch/issues/31)) ([741b05e](https://github.com/koumoe/cli-switch/commit/741b05ebb93f33d40bf9b2af50e31d39bdeeaa88))
## [0.5.0](https://github.com/koumoe/cli-switch/compare/v0.4.5...v0.5.0) (2025-12-21)

### Features

* **macos:** hide dock icon when minimized to tray ([#29](https://github.com/koumoe/cli-switch/issues/29)) ([902db22](https://github.com/koumoe/cli-switch/commit/902db220fc4de3a946eab041e82a88b5cf11cf99))

### Bug Fixes

* add endpoint and purpose to request logs ([#30](https://github.com/koumoe/cli-switch/issues/30)) ([e4b0e3c](https://github.com/koumoe/cli-switch/commit/e4b0e3c91b1a296e255926796df4b5191c114245))
## [0.4.5](https://github.com/koumoe/cli-switch/compare/v0.4.4...v0.4.5) (2025-12-21)

### Bug Fixes

* ignore anthropic count_tokens errors ([#28](https://github.com/koumoe/cli-switch/issues/28)) ([8c935bd](https://github.com/koumoe/cli-switch/commit/8c935bd661bbee7fce92c7eb30eaf5a2d1a1d6e7))
* reduce update-ready prompt delay ([#27](https://github.com/koumoe/cli-switch/issues/27)) ([2169591](https://github.com/koumoe/cli-switch/commit/216959138fdac0548cc87992f8ac01b25ad47bc4))
## [0.4.4](https://github.com/koumoe/cli-switch/compare/v0.4.3...v0.4.4) (2025-12-21)
## [0.4.3](https://github.com/koumoe/cli-switch/compare/v0.4.2...v0.4.3) (2025-12-21)
## [0.4.2](https://github.com/koumoe/cli-switch/compare/v0.4.1...v0.4.2) (2025-12-21)

### Bug Fixes

* handle compressed upstream responses ([813f510](https://github.com/koumoe/cli-switch/commit/813f510ea9ca4fc4cc91992e22f3b80deecb2c50))
## [0.4.1](https://github.com/koumoe/cli-switch/compare/v0.4.0...v0.4.1) (2025-12-21)

### Bug Fixes

* rotate log files by local date ([#23](https://github.com/koumoe/cli-switch/issues/23)) ([73faf89](https://github.com/koumoe/cli-switch/commit/73faf89e82b71fbb320b8b0bf34f82ed1e8f3918))
## [0.4.0](https://github.com/koumoe/cli-switch/compare/v0.3.1...v0.4.0) (2025-12-20)

### Features

* **logging:** add structured logging, date-range picker, and cleanup APIs ([#22](https://github.com/koumoe/cli-switch/issues/22)) ([37bad59](https://github.com/koumoe/cli-switch/commit/37bad596f63b07900df1bacafee4261d643da871))
## [0.3.1](https://github.com/koumoe/cli-switch/compare/v0.3.0...v0.3.1) (2025-12-20)

### Bug Fixes

* relaunch app after applying update ([6bbd98a](https://github.com/koumoe/cli-switch/commit/6bbd98a3c0be923d560518e291606f5c9b76755f))
* satisfy clippy in updater relaunch ([ef9769d](https://github.com/koumoe/cli-switch/commit/ef9769d25b25e0d0b9a416198878cafc2dd0fd27))
## [0.3.0](https://github.com/koumoe/cli-switch/compare/v0.2.9...v0.3.0) (2025-12-20)

### Features

* **maintenance:** add record clearing and db size APIs ([97a75a9](https://github.com/koumoe/cli-switch/commit/97a75a912e765e3547df3af000ad7896ca991e23))
* **ui:** add settings record clearing and db size display ([97a82d9](https://github.com/koumoe/cli-switch/commit/97a82d9e9c71ae27249d13112a8d01fca44a887d))

### Bug Fixes

* adjust overview layout and distribution view ([8c17259](https://github.com/koumoe/cli-switch/commit/8c1725967e7c8dd976d077aab51c28ab8f4e924a))
## [0.2.9](https://github.com/koumoe/cli-switch/compare/v0.2.8...v0.2.9) (2025-12-19)

### Bug Fixes

* stop update check from mis-triggering downloads ([#18](https://github.com/koumoe/cli-switch/issues/18)) ([5f4bd7a](https://github.com/koumoe/cli-switch/commit/5f4bd7ae46ef2b304a3eb55774668813d4168a12))
## [0.2.8](https://github.com/koumoe/cli-switch/compare/v0.2.6...v0.2.8) (2025-12-19)

### Bug Fixes

* **ci:** base next version on Cargo.toml when ahead of tags ([1e3ed3f](https://github.com/koumoe/cli-switch/commit/1e3ed3f69cc635820cbd15f783c7d321562d6c4d))
* **ci:** read commit message safely ([6649696](https://github.com/koumoe/cli-switch/commit/6649696ec173e2dbdd78b104b1bf0e9383aa71a6))
* **macos:** ad-hoc sign app bundle ([ec73432](https://github.com/koumoe/cli-switch/commit/ec7343210dcd1f9bb128a4a543a311623b23666e))
* **macos:** re-sign app after self-update ([643845d](https://github.com/koumoe/cli-switch/commit/643845d98ad7606466c19b3f3019bf1aa3affbb7))
* **update:** show server version and download progress ([#12](https://github.com/koumoe/cli-switch/issues/12)) ([d035384](https://github.com/koumoe/cli-switch/commit/d0353845d4e283711f519431a67c0ff545a3eda6))
## [0.2.7](https://github.com/koumoe/cli-switch/compare/v0.2.6...v0.2.7) (2025-12-19)

### Bug Fixes

* **macos:** ad-hoc sign app bundle ([ec73432](https://github.com/koumoe/cli-switch/commit/ec7343210dcd1f9bb128a4a543a311623b23666e))
* **macos:** re-sign app after self-update ([643845d](https://github.com/koumoe/cli-switch/commit/643845d98ad7606466c19b3f3019bf1aa3affbb7))
* **update:** show server version and download progress ([#12](https://github.com/koumoe/cli-switch/issues/12)) ([d035384](https://github.com/koumoe/cli-switch/commit/d0353845d4e283711f519431a67c0ff545a3eda6))
## [0.2.6](https://github.com/koumoe/cli-switch/compare/v0.2.5...v0.2.6) (2025-12-19)

### Bug Fixes

* log Gemini model and estimated cost ([31929e0](https://github.com/koumoe/cli-switch/commit/31929e01ddec8e4440463f85886f499b8b666279))
* satisfy rustfmt in Gemini log test ([57b7050](https://github.com/koumoe/cli-switch/commit/57b70504114391b1d9932fae63e3347d638e385a))
## [0.2.5](https://github.com/koumoe/cli-switch/compare/v0.2.4...v0.2.5) (2025-12-19)

### Bug Fixes

* **ci:** replace semantic-release with commit analyzer ([#10](https://github.com/koumoe/cli-switch/issues/10)) ([5e036cc](https://github.com/koumoe/cli-switch/commit/5e036cc8ddd6c39a06f234f48bb6f01dbf1a52be))
## [0.2.4](https://github.com/koumoe/cli-switch/compare/v0.2.3...v0.2.4) (2025-12-19)

### Bug Fixes

* **ci:** stabilize release workflow ([b2ea369](https://github.com/koumoe/cli-switch/commit/b2ea369f470cc2930fbb456c042628b1fa30867c))
* **release:** repair changelog ([aec61a8](https://github.com/koumoe/cli-switch/commit/aec61a8e0840ccc2f00a8158127a587d6e8642a0))
## [0.2.3](https://github.com/koumoe/cli-switch/compare/v0.2.2...v0.2.3) (2025-12-19)

### Bug Fixes

* **ci:** create temp package.json for changelog version ([#8](https://github.com/koumoe/cli-switch/issues/8)) ([b17eed7](https://github.com/koumoe/cli-switch/commit/b17eed7c5db08c7b251e939fbdf6229e5d878dc9))
## [0.2.2](https://github.com/koumoe/cli-switch/compare/v0.2.1...v0.2.2) (2025-12-19)

### Bug Fixes

* **ci:** prevent release workflow loop and fix changelog ([#7](https://github.com/koumoe/cli-switch/issues/7)) ([7bbb36d](https://github.com/koumoe/cli-switch/commit/7bbb36dc6c14597c989ec8fc4444153deb6f92bf))
## [0.2.1](https://github.com/koumoe/cli-switch/compare/v0.2.0...v0.2.1) (2025-12-19)
## [0.2.0](https://github.com/koumoe/cli-switch/compare/v0.1.1...v0.2.0) (2025-12-19)

### Features

* add auto-update, auto-start and improve desktop experience ([d6b2531](https://github.com/koumoe/cli-switch/commit/d6b253195988a8ba39eb13f324d3472963456749))
* add channel auto-disable on repeated failures ([df902fc](https://github.com/koumoe/cli-switch/commit/df902fc3e4e60077447fdf58c71545b08311087a))
* add channel priority, reorder, and failover ([5ec41bb](https://github.com/koumoe/cli-switch/commit/5ec41bb8b775b85ab5409351a30dee7d44aabec4))
* add logs filtering and pricing settings UI ([46cc5b9](https://github.com/koumoe/cli-switch/commit/46cc5b91d801d2ae08b9d6c9042b9243dc05628d))
* add request_id correlation for usage events ([3c40f89](https://github.com/koumoe/cli-switch/commit/3c40f89455dbafb947adf8e6469a3081a92f596e))
* add system tray and configurable close behavior ([b3f3667](https://github.com/koumoe/cli-switch/commit/b3f3667efe97eb12de23f659d12b224d3ac4e375))
* add usage list API and pricing auto sync ([79c629f](https://github.com/koumoe/cli-switch/commit/79c629fbec75fb51558b31d2078088453d47f01e))
* disable window maximize and resize ([ff3ce75](https://github.com/koumoe/cli-switch/commit/ff3ce757162b75bd669bf8ecab85ca246677fa4d))
* **pricing:** sync llm-metadata pricing with cache rates ([3f7a56c](https://github.com/koumoe/cli-switch/commit/3f7a56ce057cc6442f7298939f605f7af993c529))
* **release:** add semantic-release automation ([6a9f21e](https://github.com/koumoe/cli-switch/commit/6a9f21ebe6b2c8e5edd1347af1dca3433a2b85f6))
* ship desktop app bundles ([be63577](https://github.com/koumoe/cli-switch/commit/be63577dcd7d7cd1e44ca8f84bf7f91c56a7caf4))
* **ui:** add cost to channel stats ([3bcc244](https://github.com/koumoe/cli-switch/commit/3bcc24421a0385957104396f3abbfa8dd922075d))
* **ui:** show cache tokens in log details ([f11dd2d](https://github.com/koumoe/cli-switch/commit/f11dd2d767a2156f2eb291ebfa0bd169799e4ed7))
* **ui:** show protocol badges and i18n labels ([8e747c6](https://github.com/koumoe/cli-switch/commit/8e747c6e748d4063e0f78e1812e586ee4c939cc6))
* update overview monthly stats and trends ([62d1ed0](https://github.com/koumoe/cli-switch/commit/62d1ed07c908999780144a14d6b1233ab640a6d9))

### Bug Fixes

* **ci:** remove invalid secrets check in job condition ([69e3cc8](https://github.com/koumoe/cli-switch/commit/69e3cc8072f147a9769ff417afe1e4890766dc3a))
* **ci:** skip semantic-release without token ([abc9964](https://github.com/koumoe/cli-switch/commit/abc9964173d5e8aefa6b938bf63b83ad35208391))
* **proxy:** enrich upstream error details ([f4052eb](https://github.com/koumoe/cli-switch/commit/f4052eb1a31a0686ab26a0a08172c3f4f4d13554))
* **release:** accept prerelease tag format ([ad489ed](https://github.com/koumoe/cli-switch/commit/ad489ed06636c738ec2830d58f231c320032f43f))
* **release:** restore version validation and CI gate ([34480e0](https://github.com/koumoe/cli-switch/commit/34480e03672dc8c5bd25380b993ad0e5dd8004a5))
* satisfy CI checks ([400fdf5](https://github.com/koumoe/cli-switch/commit/400fdf5481d651fb37271553ddf5847d8e193312))
* **ui:** add bottom padding to pages ([661567c](https://github.com/koumoe/cli-switch/commit/661567c360167630ee75225d8dd58286c716672d))
* **ui:** improve logs details and error messages ([46dea19](https://github.com/koumoe/cli-switch/commit/46dea1927ca06fb5e22d83a95c61f6ee537ad5fe))
* **ui:** refine logs table layout ([f283e1b](https://github.com/koumoe/cli-switch/commit/f283e1baf1988ace3fca9e68e57487dd3647c91e))
* **windows:** render apply script without format! ([ef0bff9](https://github.com/koumoe/cli-switch/commit/ef0bff94077e8839067227abdc4081ec889c6a58))
## [0.1.1](https://github.com/koumoe/cli-switch/compare/e46c8238f56bad8652072d2cdd62aa39f8db40fa...v0.1.1) (2025-12-17)

### Features

* add backend APIs, usage tracking and desktop mode ([8b55d14](https://github.com/koumoe/cli-switch/commit/8b55d144efcf19f9372cc50273c4562d13788464))
* add Edit menu with clipboard shortcuts for macOS ([8ddd257](https://github.com/koumoe/cli-switch/commit/8ddd25761ab16c091a93a7a2744a11ff26fb596d))
* add logs page and hide routes ([f0b3fcd](https://github.com/koumoe/cli-switch/commit/f0b3fcd61d528e7b1e89825421018e52924e115c))
* add multi-platform release workflow ([c1e5eb6](https://github.com/koumoe/cli-switch/commit/c1e5eb65709604c3a33f6ee8c34d6d9a4c2ae70a))
* add upstream proxy forwarding for OpenAI/Anthropic/Gemini ([ae05b58](https://github.com/koumoe/cli-switch/commit/ae05b58d76c9f8d3a5532fabad49a941fa807c27))
* automatic auth and terminal-based channels ([263b182](https://github.com/koumoe/cli-switch/commit/263b1826a9b1a4be5c0df539d2a6a3da61528b00))
* enrich /api/health with runtime details ([6c31456](https://github.com/koumoe/cli-switch/commit/6c3145671be0cd036d1957c038720d90428f0a81))
* implement complete web UI with SPA routing ([7f5dc23](https://github.com/koumoe/cli-switch/commit/7f5dc231ad8d3746f69d0016faed08421665ec37))
* improve desktop window and UI layout ([ee87465](https://github.com/koumoe/cli-switch/commit/ee87465ec741d95db152a34889502e6a92bea5c3))
* initial project import ([e46c823](https://github.com/koumoe/cli-switch/commit/e46c8238f56bad8652072d2cdd62aa39f8db40fa))
* record ttft and token usage in logs ([b672515](https://github.com/koumoe/cli-switch/commit/b672515cc26c160958b0e7fe17334820e3e4aea2))
* revamp web UI with Radix and Tailwind ([ba1d626](https://github.com/koumoe/cli-switch/commit/ba1d626f362b3a54f1ee9e3abb99212f3569571d))
* **ui:** add i18n with locale switch ([eddb65e](https://github.com/koumoe/cli-switch/commit/eddb65e50334231e3bdb7496eb600d25d0bfa337))

### Bug Fixes

* format rust sources ([a351b2c](https://github.com/koumoe/cli-switch/commit/a351b2c6d1880eb56f29b07a27fa245f3f9196a4))
* gate release and fix desktop builds ([c46673b](https://github.com/koumoe/cli-switch/commit/c46673b9034bbe8603ddd7cb4f57a24497e0f4ae))
* improve CI/CD workflow and release package naming ([9b10cea](https://github.com/koumoe/cli-switch/commit/9b10ceaf44db106eb911ca0a4ce74532169574e8))
* make linux arm64 desktop self-hosted ([9289456](https://github.com/koumoe/cli-switch/commit/928945644a2dfa5eb831d533b59d4206931ae0d1))
* reduce macOS debug system log noise ([4bf8fe0](https://github.com/koumoe/cli-switch/commit/4bf8fe00437c9204f20dc9668c6e1f89f02a939d))
* resolve build errors in server and storage ([7bc9dc3](https://github.com/koumoe/cli-switch/commit/7bc9dc3a253f32e90b03f4dfe0b52f9965546323))
* **ui:** fix delete confirmation in webview ([90196b5](https://github.com/koumoe/cli-switch/commit/90196b53539d9c76aaf74c7babed43daa501c7ed))
