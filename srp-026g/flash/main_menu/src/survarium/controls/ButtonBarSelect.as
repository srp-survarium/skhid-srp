package survarium.controls
{
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   
   public class ButtonBarSelect extends ButtonBar
   {
      
      protected var _focusIndex:int = 1;
      
      public function ButtonBarSelect()
      {
         super();
      }
      
      public function set focusIndex(value:int) : void
      {
         if(value == this._focusIndex)
         {
            return;
         }
         var lastFocusIndex:int = this._focusIndex;
         var renderer:Button = _renderers[lastFocusIndex] as Button;
         if(renderer)
         {
            renderer.displayFocus = false;
         }
         _focusIndex = value;
         renderer = _renderers[this._focusIndex] as Button;
         if(renderer)
         {
            renderer.displayFocus = true;
         }
      }
      
      override protected function changeFocus() : void
      {
         if(!_focused)
         {
            focusIndex = selectedIndex;
         }
      }
      
      override public function set selectedIndex(value:int) : void
      {
         var lastIndex:int;
         if(value == _selectedIndex)
         {
            return;
         }
         lastIndex = _selectedIndex;
         var renderer:Button = _renderers[lastIndex] as Button;
         if(renderer)
         {
            renderer.selected = false;
         }
         _selectedIndex = value;
         focusIndex = value;
         renderer = _renderers[_selectedIndex] as Button;
         if(renderer)
         {
            renderer.selected = true;
         }
         dispatchEvent(new IndexEvent(IndexEvent.INDEX_CHANGE,true,true,_selectedIndex,lastIndex,_dataProvider[_selectedIndex]));
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.handled)
         {
            return;
         }
         var renderer:Button = _renderers[this._focusIndex] as Button;
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
         var nextIndexValid:Boolean = false;
         var nextIndex:Number;
         switch(details.navEquivalent)
         {
            case NavigationCode.LEFT:
               if(_direction == DIRECTION_HORIZONTAL)
               {
                  nextIndex = this._focusIndex - 1;
                  nextIndexValid = true;
               }
               break;
            case NavigationCode.RIGHT:
               if(_direction == DIRECTION_HORIZONTAL)
               {
                  nextIndex = this._focusIndex + 1;
                  nextIndexValid = true;
               }
               break;
            case NavigationCode.UP:
               if(_direction == DIRECTION_VERTICAL)
               {
                  nextIndex = this._focusIndex - 1;
                  nextIndexValid = true;
               }
               break;
            case NavigationCode.DOWN:
               if(_direction == DIRECTION_VERTICAL)
               {
                  nextIndex = this._focusIndex + 1;
                  nextIndexValid = true;
               }
               break;
            default:
               break;
         }
         if(nextIndexValid)
         {
            nextIndex = Math.max(0,Math.min(_dataProvider.length - 1,nextIndex));
            if(nextIndex != this._focusIndex)
            {
               focusIndex = nextIndex;
               event.handled = true;
            }
         }
      }
   }
}

