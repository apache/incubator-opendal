// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use std::fmt::Debug;

use asyncband::once::OnceCell;
use mongodb::bson::Binary;
use mongodb::bson::Bson;
use mongodb::bson::Document;
use mongodb::bson::doc;
use mongodb::options::ClientOptions;

use opendal_core::*;

#[derive(Clone)]
pub struct MongodbCore {
    pub connection_string: String,
    pub database: String,
    pub collection: String,
    pub collection_instance: OnceCell<mongodb::Collection<Document>>,
    pub key_field: String,
    pub value_field: String,
}

impl Debug for MongodbCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MongodbCore")
            .field("database", &self.database)
            .field("collection", &self.collection)
            .field("key_field", &self.key_field)
            .field("value_field", &self.value_field)
            .finish_non_exhaustive()
    }
}

impl MongodbCore {
    async fn get_collection(&self) -> Result<&mongodb::Collection<Document>> {
        self.collection_instance
            .get_or_try_init(|| async {
                let client_options = ClientOptions::parse(&self.connection_string)
                    .await
                    .map_err(parse_mongodb_error)?;
                let client =
                    mongodb::Client::with_options(client_options).map_err(parse_mongodb_error)?;
                let database = client.database(&self.database);
                let collection = database.collection(&self.collection);
                Ok(collection)
            })
            .await
    }

    pub async fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let collection = self.get_collection().await?;
        let filter = doc! {self.key_field.as_str():path};
        let result = collection
            .find_one(filter)
            .await
            .map_err(parse_mongodb_error)?;
        match result {
            Some(doc) => {
                let value = doc
                    .get_binary_generic(&self.value_field)
                    .map_err(parse_bson_error)?;
                Ok(Some(Buffer::from(value.to_vec())))
            }
            None => Ok(None),
        }
    }

    pub async fn get_length(&self, path: &str) -> Result<Option<usize>> {
        let collection = self.get_collection().await?;
        let mut cursor = collection
            .aggregate(vec![
                doc! { "$match": { self.key_field.as_str(): path } },
                doc! { "$limit": 1 },
                doc! {
                    "$project": {
                        "_id": 0,
                        "content_length": {
                            "$binarySize": format!("${}", self.value_field)
                        }
                    }
                },
            ])
            .await
            .map_err(parse_mongodb_error)?;

        if !cursor.advance().await.map_err(parse_mongodb_error)? {
            return Ok(None);
        }

        let doc: Document = cursor.deserialize_current().map_err(parse_mongodb_error)?;
        parse_bson_usize(doc.get("content_length"))
    }

    pub async fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let collection = self.get_collection().await?;
        let filter = doc! { self.key_field.as_str(): path };
        let update = doc! { "$set": { self.value_field.as_str(): Binary { subtype: mongodb::bson::spec::BinarySubtype::Generic, bytes: value.to_vec() } } };
        collection
            .update_one(filter, update)
            .upsert(true)
            .await
            .map_err(parse_mongodb_error)?;

        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let collection = self.get_collection().await?;
        let filter = doc! {self.key_field.as_str():path};
        collection
            .delete_one(filter)
            .await
            .map_err(parse_mongodb_error)?;
        Ok(())
    }
}

fn parse_mongodb_error(err: mongodb::error::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "mongodb error").set_source(err)
}

fn parse_bson_error(err: mongodb::bson::document::ValueAccessError) -> Error {
    Error::new(ErrorKind::Unexpected, "bson error").set_source(err)
}

fn parse_bson_usize(value: Option<&Bson>) -> Result<Option<usize>> {
    let Some(value) = value else {
        return Ok(None);
    };

    match value {
        Bson::Int32(v) => (*v).try_into().map(Some).map_err(|err| {
            Error::new(ErrorKind::Unexpected, "mongodb value length is invalid").set_source(err)
        }),
        Bson::Int64(v) => (*v).try_into().map(Some).map_err(|err| {
            Error::new(ErrorKind::Unexpected, "mongodb value length is invalid").set_source(err)
        }),
        _ => Err(Error::new(
            ErrorKind::Unexpected,
            "mongodb value length is invalid",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bson_usize() {
        assert_eq!(parse_bson_usize(Some(&Bson::Int32(6))).unwrap(), Some(6));
        assert_eq!(parse_bson_usize(Some(&Bson::Int64(6))).unwrap(), Some(6));
        assert_eq!(parse_bson_usize(None).unwrap(), None);
        assert!(parse_bson_usize(Some(&Bson::Int32(-1))).is_err());
        assert!(parse_bson_usize(Some(&Bson::String("6".to_string()))).is_err());
    }
}
