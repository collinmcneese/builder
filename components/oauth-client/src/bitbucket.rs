// Copyright (c) 2018 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::iter::FromIterator;

use serde_json;

use reqwest::{header::HeaderMap,
              Body};

use builder_core::http_client::{HttpClient,
                                ACCEPT_APPLICATION_JSON,
                                CONTENT_TYPE_FORM_URL_ENCODED};

use crate::{config::OAuth2Cfg,
            error::{Error,
                    Result},
            types::*};

pub struct Bitbucket;

#[derive(Deserialize)]
struct AuthOk {
    pub access_token: String,
}

#[derive(Deserialize)]
pub struct UserOk {
    pub user: User,
}

#[derive(Deserialize)]
pub struct User {
    pub username: String,
}

impl Bitbucket {
    fn user(&self, config: &OAuth2Cfg, client: &HttpClient, token: &str) -> Result<OAuth2User> {
        let header_values = vec![ACCEPT_APPLICATION_JSON.clone(),];
        let headers = HeaderMap::from_iter(header_values.into_iter());

        let mut resp = client.get(&config.userinfo_url)
                             .headers(headers)
                             .bearer_auth(token)
                             .send()
                             .map_err(Error::HttpClient)?;

        let body = resp.text().map_err(Error::HttpClient)?;
        debug!("Bitbucket response body: {}", body);

        if resp.status().is_success() {
            let user_ok = match serde_json::from_str::<UserOk>(&body) {
                Ok(msg) => msg,
                Err(e) => return Err(Error::Serialization(e)),
            };

            Ok(OAuth2User { id:       user_ok.user.username.to_string(),
                            username: user_ok.user.username.to_string(),
                            email:    None, })
        } else {
            Err(Error::HttpResponse(resp.status(), body))
        }
    }
}

impl OAuth2Provider for Bitbucket {
    fn authenticate(&self,
                    config: &OAuth2Cfg,
                    client: &HttpClient,
                    code: &str)
                    -> Result<(String, OAuth2User)> {
        let url = config.token_url.to_string();
        let body = format!("grant_type=authorization_code&code={}", code);

        let header_values = vec![ACCEPT_APPLICATION_JSON.clone(),
                                 CONTENT_TYPE_FORM_URL_ENCODED.clone()];
        let headers = HeaderMap::from_iter(header_values.into_iter());

        let body: Body = body.into();

        let mut resp = client.post(&url)
                             .headers(headers)
                             .body(body)
                             .basic_auth(&config.client_id[..], Some(&config.client_secret[..]))
                             .send()
                             .map_err(Error::HttpClient)?;

        let body = resp.text().map_err(Error::HttpClient)?;
        debug!("Bitbucket response body: {}", body);

        let token = if resp.status().is_success() {
            match serde_json::from_str::<AuthOk>(&body) {
                Ok(msg) => msg.access_token,
                Err(e) => return Err(Error::Serialization(e)),
            }
        } else {
            return Err(Error::HttpResponse(resp.status(), body));
        };

        let user = self.user(config, client, &token)?;
        Ok((token, user))
    }
}
