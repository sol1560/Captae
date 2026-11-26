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
 * Medcxy 改过：从 catalog app/.../catalog/utils/DampedDragAnimation.kt 复制；改了包名，awaitFrame 直接用 kotlinx.coroutines.android 的，类改成 internal。
 *
 * Captae 改过：从 medcxy-v7 apps/android/glass 搬来（那边的改动说明见上一段），包名改成 app.captae.glass。
 */
package app.captae.glass.core

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.MutatorMutex
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.unit.IntSize
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.android.awaitFrame
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.time.Clock

internal class DampedDragAnimation(
  private val animationScope: CoroutineScope,
  val initialValue: Float,
  val valueRange: ClosedRange<Float>,
  val visibilityThreshold: Float,
  val initialScale: Float,
  val pressedScale: Float,
  val onDragStarted: DampedDragAnimation.(position: Offset) -> Unit,
  val onDragStopped: DampedDragAnimation.() -> Unit,
  val onDrag: DampedDragAnimation.(size: IntSize, dragAmount: Offset) -> Unit,
) {

  private val valueAnimationSpec =
    spring(1f, 1000f, visibilityThreshold)
  private val velocityAnimationSpec =
    spring(0.5f, 300f, visibilityThreshold * 10f)
  private val pressProgressAnimationSpec =
    spring(1f, 1000f, 0.001f)
  private val scaleXAnimationSpec =
    spring(0.6f, 250f, 0.001f)
  private val scaleYAnimationSpec =
    spring(0.7f, 250f, 0.001f)

  private val valueAnimation =
    Animatable(initialValue, visibilityThreshold)
  private val velocityAnimation =
    Animatable(0f, 5f)
  private val pressProgressAnimation =
    Animatable(0f, 0.001f)
  private val scaleXAnimation =
    Animatable(initialScale, 0.001f)
  private val scaleYAnimation =
    Animatable(initialScale, 0.001f)

  private val mutatorMutex = MutatorMutex()

  private val velocityTracker = VelocityTracker()

  val value: Float get() = valueAnimation.value
  val progress: Float get() = (value - valueRange.start) / (valueRange.endInclusive - valueRange.start)
  val targetValue: Float get() = valueAnimation.targetValue
  val pressProgress: Float get() = pressProgressAnimation.value
  val scaleX: Float get() = scaleXAnimation.value
  val scaleY: Float get() = scaleYAnimation.value
  val velocity: Float get() = velocityAnimation.value

  val modifier: Modifier = Modifier.pointerInput(Unit) {
    inspectDragGestures(
      onDragStart = { down ->
        onDragStarted(down.position)
        press()
      },
      onDragEnd = {
        onDragStopped()
        release()
      },
      onDragCancel = {
        onDragStopped()
        release()
      }
    ) { change, dragAmount ->
      onDrag(size, dragAmount)
    }
  }

  fun press() {
    velocityTracker.resetTracking()
    animationScope.launch {
      launch { pressProgressAnimation.animateTo(1f, pressProgressAnimationSpec) }
      launch { scaleXAnimation.animateTo(pressedScale, scaleXAnimationSpec) }
      launch { scaleYAnimation.animateTo(pressedScale, scaleYAnimationSpec) }
    }
  }

  fun release() {
    animationScope.launch {
      awaitFrame()
      if (value != targetValue) {
        val threshold = (valueRange.endInclusive - valueRange.start) * 0.025f
        snapshotFlow { valueAnimation.value }
          .filter { abs(it - valueAnimation.targetValue) < threshold }
          .first()
      }
      launch { pressProgressAnimation.animateTo(0f, pressProgressAnimationSpec) }
      launch { scaleXAnimation.animateTo(initialScale, scaleXAnimationSpec) }
      launch { scaleYAnimation.animateTo(initialScale, scaleYAnimationSpec) }
    }
  }

  // Captae 改过（动效评审第 21 条）：拖动中透镜 1:1 贴着手指（snapTo），不再用弹簧追（时间常数约 32ms，快拖时落后一两帧）；
  // 速度照旧按手指的位置算（拉伸效果、松手判断用）
  fun updateValue(value: Float) {
    val targetValue = value.coerceIn(valueRange)
    animationScope.launch {
      valueAnimation.snapTo(targetValue)
      updateVelocity()
    }
  }

  /** 松手时手指的速度（格 / 秒），选标签时看甩的方向（第 22 条） */
  fun releaseVelocity(): Float = velocityTracker.calculateVelocity().x

  fun animateToValue(value: Float) {
    animationScope.launch {
      mutatorMutex.mutate {
        press()
        val targetValue = value.coerceIn(valueRange)
        launch { valueAnimation.animateTo(targetValue, valueAnimationSpec) }
        if (velocity != 0f) {
          launch { velocityAnimation.animateTo(0f, velocityAnimationSpec) }
        }
        release()
      }
    }
  }

  private fun updateVelocity() {
    velocityTracker.addPosition(
      Clock.System.now().toEpochMilliseconds(),
      Offset(value, 0f)
    )
    val targetVelocity = velocityTracker.calculateVelocity().x / (valueRange.endInclusive - valueRange.start)
    animationScope.launch { velocityAnimation.animateTo(targetVelocity, velocityAnimationSpec) }
  }
}
