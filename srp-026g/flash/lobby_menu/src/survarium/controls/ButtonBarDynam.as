package survarium.controls
{
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   
   public class ButtonBarDynam extends ButtonBar
   {
      
      public var useRect:Boolean = true;
      
      public function ButtonBarDynam()
      {
         super();
      }
      
      override protected function updateRenderers() : void
      {
         var offsetX:Number = 0;
         var offsetY:Number = 0;
         var overflowIndex:int = -1;
         if(_renderers[0] is _itemRendererClass)
         {
            while(_renderers.length > _dataProvider.length)
            {
               lastIndex = _renderers.length - 1;
               if(container.contains(_renderers[lastIndex]))
               {
                  container.removeChild(_renderers[lastIndex]);
               }
               _renderers.splice(lastIndex--,1);
            }
         }
         else
         {
            while(container.numChildren > 0)
            {
               container.removeChildAt(0);
            }
            _renderers.length = 0;
         }
         var i:uint = 0;
         var lastIndex:int;
         var renderer:Button;
         var isNew:Boolean;
         var j:int;
         var extraRenderer:Button;
         while(i < _dataProvider.length && overflowIndex == -1)
         {
            isNew = false;
            if(i < _renderers.length)
            {
               renderer = _renderers[i];
            }
            else
            {
               renderer = new _itemRendererClass();
               setupRenderer(renderer,i);
               isNew = true;
            }
            populateRendererData(renderer,i);
            if(_autoSize == TextFieldAutoSize.NONE && _buttonWidth > 0)
            {
               renderer.width = _buttonWidth;
            }
            else if(_autoSize != TextFieldAutoSize.NONE)
            {
               renderer.autoSize = _autoSize;
            }
            renderer.validateNow();
            if(_direction == DIRECTION_HORIZONTAL)
            {
               if(_width > renderer.width + _spacing + offsetX || _maxFit)
               {
                  renderer.y = 0;
                  renderer.x = offsetX;
                  offsetX += (this.useRect ? renderer.getRect(renderer).width : renderer.width) + _spacing;
               }
               else
               {
                  overflowIndex = i;
                  renderer = null;
               }
            }
            else if(_height > renderer.height + _spacing + offsetY || _maxFit)
            {
               renderer.x = 0;
               renderer.y = offsetY;
               offsetY += renderer.getRect(renderer).height + _spacing;
            }
            else
            {
               overflowIndex = i;
               renderer = null;
            }
            if(isNew && renderer != null)
            {
               renderer.group = _group;
               container.addChild(renderer);
               _renderers.push(renderer);
            }
            i++;
         }
         if(overflowIndex > -1)
         {
            j = _renderers.length - 1;
            while(j >= overflowIndex)
            {
               extraRenderer = _renderers[j];
               if(extraRenderer)
               {
                  if(container.contains(extraRenderer))
                  {
                     container.removeChild(extraRenderer);
                  }
                  _renderers.splice(j,1);
               }
               j--;
            }
         }
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
      }
   }
}

