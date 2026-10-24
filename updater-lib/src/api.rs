use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use common::{AppEntry, AppMeta, Release};
use reqwest::blocking::{multipart, Client, Response};
use sha2::{Digest, Sha256};

pub struct Api {
    base: String,
    client: Client,
    dev_password: Option<String>,
}

impl Api {
    pub fn new(base: &str) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(None)
            .build()
            .expect("http client");
        Self { base: base.trim_end_matches('/').to_string(), client, dev_password: None }
    }

    pub fn with_password(mut self, password: Option<String>) -> Self {
        self.dev_password = password;
        self
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn dev(&self, builder: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        builder.header("x-dev-password", self.dev_password.clone().unwrap_or_default())
    }

    pub fn list_apps(&self) -> Result<Vec<AppEntry>> {
        let resp = ok(self.client.get(self.url("/api/apps")).send().context("brak polaczenia z serwerem")?)?;
        Ok(resp.json()?)
    }

    pub fn download(
        &self,
        app_id: &str,
        release: &Release,
        dest: &Path,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<()> {
        let path = format!("/api/apps/{}/{}/{}/download", app_id, release.version, release.platform);
        let mut resp = ok(self.client.get(self.url(&path)).send()?)?;
        let mut file = File::create(dest)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        loop {
            let n = resp.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            hasher.update(&buf[..n]);
            done += n as u64;
            progress(done, release.size);
        }
        if hex::encode(hasher.finalize()) != release.sha256 {
            bail!("niezgodna suma sha256 pobranego pliku");
        }
        Ok(())
    }

    pub fn dev_check(&self) -> Result<()> {
        ok(self.dev(self.client.get(self.url("/api/dev/check"))).send()?)?;
        Ok(())
    }

    pub fn dev_upsert_app(&self, id: &str, name: &str, description: &str) -> Result<()> {
        let meta = AppMeta { name: name.into(), description: description.into() };
        let req = self.client.put(self.url(&format!("/api/dev/apps/{id}"))).json(&meta);
        ok(self.dev(req).send()?)?;
        Ok(())
    }

    pub fn dev_upload(&self, id: &str, version: &str, platform: &str, notes: &str, file: &Path) -> Result<()> {
        let form = multipart::Form::new()
            .text("version", version.to_string())
            .text("platform", platform.to_string())
            .text("notes", notes.to_string())
            .file("file", file)?;
        let req = self.client.post(self.url(&format!("/api/dev/apps/{id}/releases"))).multipart(form);
        ok(self.dev(req).send()?)?;
        Ok(())
    }

    pub fn dev_delete_app(&self, id: &str) -> Result<()> {
        let req = self.client.delete(self.url(&format!("/api/dev/apps/{id}")));
        ok(self.dev(req).send()?)?;
        Ok(())
    }

    pub fn dev_delete_release(&self, id: &str, version: &str, platform: &str) -> Result<()> {
        let req = self.client.delete(self.url(&format!("/api/dev/apps/{id}/{version}/{platform}")));
        ok(self.dev(req).send()?)?;
        Ok(())
    }
}

fn ok(resp: Response) -> Result<Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    bail!("{status}: {body}")
}
