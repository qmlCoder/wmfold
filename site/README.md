# wmfold 官网

wmfold 的产品主页，部署在 **https://www.xiaofei911.top/statics/wmfold/**。

纯静态：一个 `index.html` + `assets/`，没有构建步骤、没有外部 CDN，
字体是本地子集化的思源系字体（OFL）。

```
site/
├── index.html              单页站点（内联 CSS/JS，无外部依赖）
├── assets/
│   ├── app-log.webp        主界面截图：高斯日志筛选 + 化学列 + 预览面板
│   ├── app-files.webp      「全部文件」视图截图
│   ├── app-settings.webp   设置 → 展示信息（按类型挑列）
│   ├── og.png              社交分享图（1200×630）
│   ├── icon-192.png        应用图标（来自 src-tauri/icons/icon.png）
│   ├── favicon.png
│   └── fonts/              Noto Serif SC / Noto Sans SC 子集（woff2，可变字重）
```

## 截图从哪来

所有截图都是 **真实运行 wmfold 0.3.15 抓的**，没有用设计稿或合成图：

- 桌面端截图：运行 `src-tauri/target/release/wmfold.exe`，通过 WebView2 的
  `--remote-debugging-port=9222`（`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`）用 CDP
  驱动窗口、截图；窗口尺寸 1800×1000（主图）与 1400×900（其余），`deviceScaleFactor=2`。
- 演示目录 `D:\QM\芳香性`：24 个真实文件（芳香性项目的 gjf / log / fch / molden /
  xyz / mol / sdf / out，最大 7.6 MB），化学列与表格里的数字都来自
  `wmfold -cli list D:\QM\芳香性 --chem` 的真实解析结果。

## 下载文件从哪来

站点**不再自己保存安装包**。三个下载按钮指向服务器发布目录：

```
服务器路径   /home/fly/share/programs/wmfold/windows/<version>/
公开路径     https://www.xiaofei911.top/statics/wmfold-updates/windows/<version>/
             （/home/fly/statics/wmfold-updates 是指向上面目录的符号链接，由 Caddy 的 /statics/* 提供）
```

每个版本目录里有：`wmfold_<v>_x64-setup.exe`（+ `.sig` 更新签名）、`wmfold_<v>.exe`、`wmfold_<v>_x64_en-US.msi`。
页面里的链接是**静态兜底**（写死当前版本），加载后会再请求
`windows/latest-version-info.json`，把三张卡片自动改写到最新版本并刷新版本号/文件大小/构建日期；
接口不可用或文件缺失时保留静态链接。发布新版本用：

```
npm run build:app
npm run publish:update -- "更新说明"
```

`publish-update.mjs` 会把安装包 + 签名 + 便携版 + MSI 一起传到
`/home/fly/share/programs/wmfold/windows/<version>/`，更新清单并校验公网可访问。
页面无需改动即可显示新版本（JSON-LD 里的版本号仍是静态的，发版后可顺手更新）。

## 改内容时注意

- 版本号、文件大小、构建日期在 `index.html` 里出现多次（导航、下载区、JSON-LD、
  页脚「当前版本」），升级版本时一起改。
- `assets/fonts/*.woff2` 是按页面实际用到的字符子集化的。**如果新增了中文文案，
  必须重新生成字体子集**，否则新字会回退到系统字体：
  `py -3 scripts/website/subset-fonts.py`（脚本从 `site/index.html` 提取字符）。
- 重新生成 webp / og 图：`py -3 scripts/website/make-assets.py`（需要先有原始截图）。
- 页面主体颜色取自应用图标（青色 + 琥珀色），标题用思源宋体、正文用思源黑体。

## 部署

```
# 1) 打包页面（Windows 自带 bsdtar；下载文件由发布目录提供，不用打包）
tar.exe -czf site.tar.gz -C site index.html assets

# 2) 上传并解压到服务器（Caddy 已把 /statics/* 映射到 /home/fly/statics）
scp site.tar.gz fly@47.115.214.219:/tmp/
ssh fly@47.115.214.219 "cd /home/fly/statics/wmfold && tar -xzf /tmp/site.tar.gz && chmod -R a+rX . && rm /tmp/site.tar.gz"
```

线上路径：`https://www.xiaofei911.top/statics/wmfold/`（`/statics/wmfold` 会 308 到带斜杠的地址）。
Caddy 配置无需改动：`www.xiaofei911.top` 站点里的 `handle_path /statics/*` 已覆盖该目录，并开启 gzip。

本地预览：

```
py -3 -m http.server 8099 --directory site
# 打开 http://127.0.0.1:8099/
```

## 页脚备案

`www.xiaofei911.top` 的备案号是 **豫ICP备2023007582号-1**，页脚已带上并链接到
工信部备案系统。
