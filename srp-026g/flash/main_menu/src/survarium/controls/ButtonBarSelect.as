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
      
      public function set focusIndex(param1:int) : void
      {
         if(param1 == this._focusIndex)
         {
            return;
         }
         var _loc2_:int = this._focusIndex;
         var _loc3_:Button = _renderers[_loc2_] as Button;
         if(_loc3_)
         {
            _loc3_.displayFocus = false;
         }
         _focusIndex = param1;
         _loc3_ = _renderers[this._focusIndex] as Button;
         if(_loc3_)
         {
            _loc3_.displayFocus = true;
         }
      }
      
      override protected function changeFocus() : void
      {
         if(!_focused)
         {
            focusIndex = selectedIndex;
         }
      }
      
      override public function set selectedIndex(param1:int) : void
      {
         var _loc2_:int;
         if(param1 == _selectedIndex)
         {
            return;
         }
         _loc2_ = _selectedIndex;
         var _loc3_:Button = _renderers[_loc2_] as Button;
         if(_loc3_)
         {
            _loc3_.selected = false;
         }
         _selectedIndex = param1;
         focusIndex = param1;
         _loc3_ = _renderers[_selectedIndex] as Button;
         if(_loc3_)
         {
            _loc3_.selected = true;
         }
         dispatchEvent(new IndexEvent(IndexEvent.INDEX_CHANGE,true,true,_selectedIndex,_loc2_,_dataProvider[_selectedIndex]));
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.handled)
         {
            return;
         }
         var _loc2_:Button = _renderers[this._focusIndex] as Button;
         if(_loc2_ != null)
         {
            _loc2_.handleInput(param1);
            if(param1.handled)
            {
               return;
            }
         }
         var _loc3_:InputDetails = param1.details;
         var _loc4_:Boolean = _loc3_.value == InputValue.KEY_DOWN || _loc3_.value == InputValue.KEY_HOLD;
         if(!_loc4_)
         {
            return;
         }
         var _loc5_:Boolean = false;
         var _loc6_:Number;
         switch(_loc3_.navEquivalent)
         {
            case NavigationCode.LEFT:
               if(_direction == DIRECTION_HORIZONTAL)
               {
                  _loc6_ = this._focusIndex - 1;
                  _loc5_ = true;
               }
               break;
            case NavigationCode.RIGHT:
               if(_direction == DIRECTION_HORIZONTAL)
               {
                  _loc6_ = this._focusIndex + 1;
                  _loc5_ = true;
               }
               break;
            case NavigationCode.UP:
               if(_direction == DIRECTION_VERTICAL)
               {
                  _loc6_ = this._focusIndex - 1;
                  _loc5_ = true;
               }
               break;
            case NavigationCode.DOWN:
               if(_direction == DIRECTION_VERTICAL)
               {
                  _loc6_ = this._focusIndex + 1;
                  _loc5_ = true;
               }
               break;
            default:
               break;
         }
         if(_loc5_)
         {
            _loc6_ = Math.max(0,Math.min(_dataProvider.length - 1,_loc6_));
            if(_loc6_ != this._focusIndex)
            {
               focusIndex = _loc6_;
               param1.handled = true;
            }
         }
      }
   }
}

