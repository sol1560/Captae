/*
 * Copyright 2025 Kyant (https://github.com/Kyant0/AndroidLiquidGlass)
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * Medcxy 改过：从 AndroidLiquidGlass catalog（kmp 分支 65ab177e）app/.../catalog/utils/InteractiveHighlight.kt 复制。
 * 改动：包名；高光画在内容前面（iOS 按下时字形被提亮发灰）；加了 enabled（禁用的按钮不回弹）；
 * 按下后另外记一个「按住」状态给 layerBlock 算放大；低于 33 没有 RuntimeShader 时只铺一层白。
 *
 * Captae 改过：从 medcxy-v7 apps/android/glass 搬来（那边的改动说明见上一段），包名改成 app.captae.glass。
 */
package app.captae.glass.core

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.VectorConverter
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.core.spring
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.util.fastCoerceIn
import com.kyant.backdrop.RuntimeShader
import com.kyant.backdrop.asComposeShader
import com.kyant.backdrop.isRuntimeShaderSupported
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

internal class InteractiveHighlight(
  val animationScope: CoroutineScope,
  val enabled: () -> Boolean = { true },
  val position: (size: Size, offset: Offset) -> Offset = { _, offset -> offset },
) {

  private val pressProgressAnimationSpec =
    spring(0.5f, 300f, 0.001f)
  private val positionAnimationSpec =
    spring(0.5f, 300f, Offset.VisibilityThreshold)

  private val pressProgressAnimation =
    Animatable(0f, 0.001f)
  private val positionAnimation =
    Animatable(Offset.Zero, Offset.VectorConverter, Offset.VisibilityThreshold)

  private var startPosition = Offset.Zero
  val pressProgress: Float get() = pressProgressAnimation.value
  val offset: Offset get() = positionAnimation.value - startPosition

  private val shader =
    if (isRuntimeShaderSupported()) {
      RuntimeShader(
        """
uniform float2 size;
layout(color) uniform half4 color;
uniform float radius;
uniform float2 position;

half4 main(float2 coord) {
  float dist = distance(coord, position);
  float intensity = smoothstep(radius, radius * 0.5, dist);
  return color * intensity;
}"""
      )
    } else {
      null
    }

  val modifier: Modifier =
    Modifier.drawWithContent {
      drawContent()
      val progress = pressProgressAnimation.value
      if (progress > 0f) {
        if (shader != null) {
          drawRect(
            Color.White.copy(0.08f * progress),
            blendMode = BlendMode.Plus
          )
          shader.apply {
            val position = position(size, positionAnimation.value)
            setFloatUniform("size", size.width, size.height)
            setColorUniform("color", Color.White.copy(0.15f * progress))
            setFloatUniform("radius", size.minDimension * 1.5f)
            setFloatUniform(
              "position",
              position.x.fastCoerceIn(0f, size.width),
              position.y.fastCoerceIn(0f, size.height)
            )
          }
          drawRect(
            ShaderBrush(shader.asComposeShader()),
            blendMode = BlendMode.Plus
          )
        } else {
          drawRect(
            Color.White.copy(0.25f * progress),
            blendMode = BlendMode.Plus
          )
        }
      }
    }

  val gestureModifier: Modifier =
    Modifier.pointerInput(animationScope) {
      inspectDragGestures(
        onDragStart = { down ->
          if (!enabled()) return@inspectDragGestures
          startPosition = down.position
          animationScope.launch {
            launch { pressProgressAnimation.animateTo(1f, pressProgressAnimationSpec) }
            launch { positionAnimation.snapTo(startPosition) }
          }
        },
        onDragEnd = {
          animationScope.launch {
            launch { pressProgressAnimation.animateTo(0f, pressProgressAnimationSpec) }
            launch { positionAnimation.animateTo(startPosition, positionAnimationSpec) }
          }
        },
        onDragCancel = {
          animationScope.launch {
            launch { pressProgressAnimation.animateTo(0f, pressProgressAnimationSpec) }
            launch { positionAnimation.animateTo(startPosition, positionAnimationSpec) }
          }
        }
      ) { change, _ ->
        if (enabled()) animationScope.launch { positionAnimation.snapTo(change.position) }
      }
    }
}
