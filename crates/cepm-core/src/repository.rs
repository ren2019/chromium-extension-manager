//! Ready extension artifacts in a GitHub repository, resolved to immutable commits.
use crate::{
    Manager, Preview, Result, UpdatePlan,
    remote::{
        Gh, GitHubRepo, MAX_ARCHIVE_BYTES, MAX_JSON_BYTES, RemoteOrigin, api_args, encode_segment,
        extract_zip,
    },
};
use std::{fs, path::Path};

impl Manager {
    /// Fetch the chosen ref at its resolved commit. No repository scripts are executed.
    pub fn preview_repository(
        &self,
        gh: &impl Gh,
        repository: &str,
        reference: &str,
        subdirectory: &str,
    ) -> Result<Preview> {
        let repo = GitHubRepo::parse(repository)?;
        let subdirectory = if subdirectory == "." {
            ""
        } else {
            subdirectory
        };
        if subdirectory.len() > 512
            || subdirectory.contains(['\\', ':'])
            || subdirectory.chars().any(char::is_control)
            || (!subdirectory.is_empty()
                && subdirectory
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == ".."))
        {
            return Err(
                "定位仓库子目录失败：请使用仓库内的相对目录，不能包含绝对路径、反斜杠或 ..".into(),
            );
        }
        if reference.is_empty() || reference.len() > 1024 || reference.chars().any(char::is_control)
        {
            return Err("解析引用失败：请填写有效的分支、标签或提交".into());
        }
        let commit_response = gh
            .run(
                &api_args(repo.endpoint(&format!("commits/{}", encode_segment(reference)))),
                MAX_JSON_BYTES,
            )
            .map_err(|e| format!("解析仓库引用失败：{e}。请检查引用及 gh 的仓库访问权限"))?;
        let response: serde_json::Value = serde_json::from_slice(&commit_response)
            .map_err(|_| "解析仓库引用失败：返回的提交信息无效，请重试".to_string())?;
        let commit = response["sha"]
            .as_str()
            .filter(|sha| sha.len() == 40 && sha.bytes().all(|c| c.is_ascii_hexdigit()))
            .ok_or("解析仓库引用失败：缺少完整提交标识，请重试")?
            .to_ascii_lowercase();
        let archive = gh
            .run(
                &api_args(repo.endpoint(&format!("zipball/{commit}"))),
                MAX_ARCHIVE_BYTES,
            )
            .map_err(|e| format!("获取仓库产物失败：{e}。请检查 gh 访问权限后重试"))?;
        let extracted = tempfile::tempdir().map_err(crate::io_error)?;
        extract_zip(&archive, extracted.path())?;
        let wrapper = fs::read_dir(extracted.path())
            .map_err(crate::io_error)?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(crate::io_error)?;
        if wrapper.len() != 1 || !wrapper[0].file_type().map_err(crate::io_error)?.is_dir() {
            return Err("定位仓库产物失败：归档应包含唯一仓库根目录，请检查来源".into());
        }
        let directory = wrapper[0].path().join(Path::new(subdirectory));
        let preview = self.preview_local(&directory).map_err(|e| format!("验证仓库子目录失败：{e}。请选择包含 manifest.json 及所需资源的预构建扩展目录；不会执行构建脚本"))?;
        Ok(preview.with_origin(RemoteOrigin::GitHubDirectory {
            repo,
            reference: reference.into(),
            commit,
            subdirectory: subdirectory.into(),
        }))
    }

    /// Re-resolve the saved branch/tag/commit and compare the resulting artifact.
    pub fn check_repository(&self, gh: &impl Gh, id: &str) -> Result<UpdatePlan> {
        let record = self.record(id)?;
        let Some(RemoteOrigin::GitHubDirectory {
            repo,
            reference,
            subdirectory,
            ..
        }) = record.origin
        else {
            return Err("此安装不是 GitHub 仓库目录来源，请选择对应的来源检查".into());
        };
        let preview = self.preview_repository(gh, &repo.slug(), &reference, &subdirectory)?;
        self.prepare_update(id, preview)
    }
}
