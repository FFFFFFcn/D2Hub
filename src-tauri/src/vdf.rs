use anyhow::{anyhow, Context, Result};
use keyvalues_parser::Vdf;
use std::fs;
use std::path::PathBuf;

/// 从 localconfig.vdf 读取 app 570 的 LaunchOptions
pub fn read_launch_options(localconfig: &PathBuf) -> Result<String> {
    let content = fs::read_to_string(localconfig).context("读取 localconfig.vdf 失败")?;
    #[allow(deprecated)]
    let vdf = Vdf::parse(&content).map_err(|e| anyhow!("VDF 解析失败: {}", e))?;

    let root = vdf
        .value
        .get_obj()
        .ok_or_else(|| anyhow!("VDF 根节点非对象"))?;

    let apps = navigate_ci(root, &["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"])
        .ok_or_else(|| anyhow!("VDF 路径不存在"))?;

    let app570 = get_obj_ci(apps, "570").ok_or_else(|| anyhow!("未找到 apps.570"))?;

    // LaunchOptions 可能存在也可能不存在
    if let Some(values) = app570.get("LaunchOptions") {
        if let Some(first) = values.first() {
            if let Some(s) = first.get_str() {
                return Ok(s.to_string());
            }
        }
    }

    Ok(String::new())
}

/// 将 app 570 的 LaunchOptions 置空（外科手术式单值替换，保持文件其余不变）
pub fn clear_launch_options(localconfig: &PathBuf) -> Result<()> {
    let content = fs::read_to_string(localconfig).context("读取失败")?;

    // 备份
    let backup = localconfig.with_extension("vdf.bak");
    fs::copy(localconfig, &backup).context("备份失败")?;

    // 读取当前值
    let current = read_launch_options(localconfig)?;
    if current.is_empty() {
        return Ok(());
    }

    // 在 570 块内外科手术式替换 LaunchOptions 行
    let new_content = replace_launch_options_in_block(&content, "570")
        .ok_or_else(|| anyhow!("未能在 570 块中定位 LaunchOptions"))?;

    fs::write(localconfig, &new_content).context("写入失败")?;

    // 回读校验
    let verify = read_launch_options(localconfig)?;
    if !verify.is_empty() {
        fs::copy(&backup, localconfig).context("回滚失败")?;
        return Err(anyhow!("写入校验失败: LaunchOptions 仍为 '{}'", verify));
    }

    Ok(())
}

fn get_obj_ci<'a>(
    obj: &'a keyvalues_parser::Obj<'a>,
    key: &str,
) -> Option<&'a keyvalues_parser::Obj<'a>> {
    for (k, v) in obj.iter() {
        if k.eq_ignore_ascii_case(key) {
            for val in v {
                if let Some(inner) = val.get_obj() {
                    return Some(inner);
                }
            }
        }
    }
    None
}

fn navigate_ci<'a>(
    obj: &'a keyvalues_parser::Obj<'a>,
    keys: &[&str],
) -> Option<&'a keyvalues_parser::Obj<'a>> {
    let mut current = obj;
    for key in keys {
        current = get_obj_ci(current, key)?;
    }
    Some(current)
}

/// 在指定 app_id 块内将 LaunchOptions 值替换为空
fn replace_launch_options_in_block(content: &str, app_id: &str) -> Option<String> {
    // 检测行结尾
    let line_ending = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let lines: Vec<&str> = if line_ending == "\r\n" {
        content.split("\r\n").collect()
    } else {
        content.lines().collect()
    };

    let mut result: Vec<String> = Vec::new();
    let mut i = 0;
    let mut replaced = false;
    let target_line = format!("\"{}\"", app_id);

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if !replaced && trimmed == target_line {
            // 检查下一行是否是 { (可能是同块开始)
            let next_is_brace = i + 1 < lines.len() && lines[i + 1].trim() == "{";
            // 也检查同行是否有 { (e.g. "570" {)
            let same_line_brace = trimmed.ends_with('{');

            if next_is_brace || same_line_brace {
                result.push(lines[i].to_string());
                i += 1;

                if next_is_brace && i < lines.len() {
                    result.push(lines[i].to_string()); // "{"
                    i += 1;
                }

                let mut depth: i32 = 1;
                while i < lines.len() && depth > 0 {
                    let t = lines[i].trim();
                    depth += t.matches('{').count() as i32;
                    depth -= t.matches('}').count() as i32;

                    if depth == 0 {
                        result.push(lines[i].to_string());
                        i += 1;
                        break;
                    }

                    if t.starts_with("\"LaunchOptions\"") {
                        let indent = &lines[i][..lines[i].len() - lines[i].trim_start().len()];
                        result.push(format!("{}\"LaunchOptions\"\t\t\"\"", indent));
                        replaced = true;
                    } else {
                        result.push(lines[i].to_string());
                    }
                    i += 1;
                }
                continue;
            }
        }

        result.push(lines[i].to_string());
        i += 1;
    }

    if replaced {
        Some(result.join(line_ending))
    } else {
        None
    }
}
