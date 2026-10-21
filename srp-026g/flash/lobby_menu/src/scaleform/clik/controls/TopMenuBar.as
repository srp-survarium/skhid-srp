package scaleform.clik.controls
{
   import flash.events.Event;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.events.ButtonBarEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   
   public class TopMenuBar extends ButtonBar
   {
      
      protected var _groupRenderers:uint = 3;
      
      protected var _groupMargin:uint = 210;
      
      public function TopMenuBar()
      {
         super();
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.handled)
         {
            return;
         }
         var renderer:Button = _renderers[_selectedIndex] as Button;
         if(renderer != null)
         {
            renderer.handleInput(event);
            if(event.handled)
            {
               return;
            }
         }
         var details:InputDetails = event.details;
         var keyPress:Boolean = details.value == InputValue.KEY_DOWN || details.value == InputValue.KEY_HOLD;
         if(!keyPress)
         {
            return;
         }
         var navigated:Boolean = false;
         var newIndex:Number;
         switch(details.navEquivalent)
         {
            case NavigationCode.LEFT:
               if(_direction == DIRECTION_HORIZONTAL)
               {
                  newIndex = this.getPrevIndex();
                  navigated = true;
               }
               break;
            case NavigationCode.RIGHT:
               if(_direction == DIRECTION_HORIZONTAL)
               {
                  newIndex = this.getNextIndex();
                  navigated = true;
               }
               break;
            case NavigationCode.UP:
               if(_direction == DIRECTION_VERTICAL)
               {
                  newIndex = this.getPrevIndex();
                  navigated = true;
               }
               break;
            case NavigationCode.DOWN:
               if(_direction == DIRECTION_VERTICAL)
               {
                  newIndex = this.getNextIndex();
                  navigated = true;
               }
               break;
            default:
               break;
         }
         if(navigated)
         {
            newIndex = Math.max(0,Math.min(_dataProvider.length - 1,newIndex));
            if(newIndex != _selectedIndex)
            {
               selectedIndex = newIndex;
               event.handled = true;
            }
         }
      }
      
      protected function getPrevIndex() : Number
      {
         var renderer:Button;
         var i:int;
         if(_selectedIndex != 0)
         {
            i = _selectedIndex - 1;
            while(i >= 0)
            {
               renderer = _renderers[i];
               if(renderer.enabled)
               {
                  return i;
               }
               i--;
            }
         }
         return _selectedIndex;
      }
      
      protected function getNextIndex() : Number
      {
         var renderer:Button;
         var i:int;
         if(_selectedIndex != _renderers.length)
         {
            i = _selectedIndex + 1;
            while(i < _renderers.length)
            {
               renderer = _renderers[i];
               if(renderer.enabled)
               {
                  return i;
               }
               i++;
            }
         }
         return _selectedIndex;
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
         var groupOffset:uint = 0;
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
                  offsetX += renderer.width + _spacing;
                  if(groupOffset != int((i + 1) / this._groupRenderers) * this._groupMargin)
                  {
                     groupOffset = int((i + 1) / this._groupRenderers) * this._groupMargin;
                     offsetX += int((i + 1) / this._groupRenderers) * this._groupMargin;
                  }
               }
               else
               {
                  overflowIndex = i;
                  renderer = null;
               }
            }
            else if(_height > renderer.height + _spacing + offsetY)
            {
               renderer.x = 0;
               renderer.y = offsetY;
               offsetY += renderer.height + _spacing;
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
      
      override protected function handleButtonGroupChange(event:Event) : void
      {
         if(_group.selectedIndex != selectedIndex)
         {
            selectedIndex = _group.selectedIndex;
            dispatchEvent(new ButtonBarEvent(ButtonBarEvent.BUTTON_SELECT,false,true,_selectedIndex,event.target as Button));
         }
         else
         {
            selectedIndex = -1;
            dispatchEvent(new ButtonBarEvent(ButtonBarEvent.BUTTON_SELECT,false,true,_selectedIndex,event.target as Button));
         }
      }
   }
}

