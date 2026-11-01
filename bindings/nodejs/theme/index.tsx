/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

import React from 'react'
import { DefaultTheme, PageEvent, Reflection, DefaultThemeRenderContext, Application, JSX } from 'typedoc'

class FooterWithASFCopyright extends DefaultThemeRenderContext {
  override footer = () => {
    return (
      <footer>
        <p>
          Copyright © 2022-{new Date().getFullYear()}, The Apache Software Foundation. Apache OpenDAL, OpenDAL, and
          Apache are either registered trademarks or trademarks of the Apache Software Foundation.
        </p>
      </footer>
    ) as unknown as JSX.Element
  }
}

class FooterOverrideTheme extends DefaultTheme {
  getRenderContext(pageEvent: PageEvent<Reflection>): DefaultThemeRenderContext {
    return new FooterWithASFCopyright(this.router, this, pageEvent, this.application.options)
  }
}

export function load(app: Application) {
  app.renderer.defineTheme('opendal', FooterOverrideTheme)
}
