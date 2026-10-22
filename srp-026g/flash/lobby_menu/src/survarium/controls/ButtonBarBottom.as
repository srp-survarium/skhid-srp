package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.events.ButtonBarEvent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   
   public class ButtonBarBottom extends ButtonBar
   {
      
      public var btn_prev:Button;
      
      public var btn_next:Button;
      
      public var buttons_background:MovieClip;
      
      protected var _toRemove:int = -1;
      
      private var _startIndex:int = 0;
      
      public function ButtonBarBottom()
      {
         super();
      }
      
      override public function set selectedIndex(value:int) : void
      {
         if(value == _selectedIndex)
         {
            return;
         }
         var oldStartIndex:int = this._startIndex;
         if(value < this._startIndex && value != -1)
         {
            _startIndex = value;
         }
         if(value > this._startIndex + _renderers.length - 1 && value != _dataProvider.length)
         {
            trace("hyer");
            _startIndex = value - _renderers.length + 1;
         }
         if(oldStartIndex != this._startIndex)
         {
            this.updateRenderers();
         }
         var lastIndex:int = _selectedIndex;
         var renderer:Button = _renderers[lastIndex - this._startIndex] as Button;
         if(renderer)
         {
            renderer.selected = false;
         }
         _selectedIndex = value;
         renderer = _renderers[_selectedIndex - this._startIndex] as Button;
         if(renderer)
         {
            renderer.selected = true;
         }
         dispatchEvent(new IndexEvent(IndexEvent.INDEX_CHANGE,true,true,_selectedIndex,lastIndex,_dataProvider[_selectedIndex]));
      }
      
      public function removeRenderer(renderer:Button) : void
      {
         var i:uint;
         while(i < _renderers.length)
         {
            if(_renderers[i] === renderer)
            {
               _toRemove = i;
               break;
            }
            i++;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.btn_prev.addEventListener(ButtonEvent.CLICK,this.arrowClick);
         this.btn_next.addEventListener(ButtonEvent.CLICK,this.arrowClick);
         this.btn_prev.visible = false;
         this.btn_next.visible = false;
      }
      
      protected function arrowClick(event:ButtonEvent) : void
      {
         if(event.target === this.btn_next)
         {
            _startIndex += 1;
         }
         else
         {
            _startIndex -= 1;
         }
         invalidateSettings();
      }
      
      protected function removeRendererAt(rendererIndex:int) : void
      {
         if(container.contains(_renderers[rendererIndex]))
         {
            container.removeChild(_renderers[rendererIndex]);
         }
         _renderers.splice(rendererIndex--,1);
      }
      
      protected function updateRenderersOnResize(event:Event) : void
      {
         this.updateRenderers();
      }
      
      override protected function handleButtonGroupChange(event:Event) : void
      {
         var buttonEvent:ButtonEvent = event as ButtonEvent;
         if(_group.selectedIndex != selectedIndex)
         {
            selectedIndex = _group.selectedIndex + this._startIndex;
            dispatchEvent(new ButtonBarEvent(ButtonBarEvent.BUTTON_SELECT,false,true,_selectedIndex,event.target as Button));
         }
      }
      
      override protected function updateRenderers() : void
      {
         var offsetX:Number = 0;
         var offsetY:Number = 0;
         var overflowIndex:int = -1;
         if(this._toRemove != -1)
         {
            this.removeRendererAt(this._toRemove);
            _toRemove = -1;
         }
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
         var requiredSize:Number;
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
               renderer.addEventListener(Event.RESIZE,this.updateRenderersOnResize);
               setupRenderer(renderer,i);
               isNew = true;
            }
            renderer.validateNow();
            if(_direction == DIRECTION_HORIZONTAL)
            {
               requiredSize = _buttonWidth + _spacing + offsetX + this.buttons_background.width;
               if(_width > _buttonWidth + _spacing + offsetX || _maxFit && _maxSize > requiredSize)
               {
                  renderer.y = 0;
                  renderer.x = offsetX;
                  offsetX += renderer.width + _spacing;
               }
               else
               {
                  overflowIndex = (i);
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
               overflowIndex = (i);
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
         if(isInvalid(InvalidationType.DATA))
         {
            if(_selectedIndex < this._startIndex && _selectedIndex != -1)
            {
               trace("lower");
               _startIndex = _selectedIndex;
            }
            if(_selectedIndex > this._startIndex + _renderers.length - 1 && _selectedIndex != _dataProvider.length)
            {
               trace("hyer");
               _startIndex = _selectedIndex - _renderers.length + 1;
            }
            if(_dataProvider.length <= _renderers.length)
            {
               _startIndex = 0;
            }
         }
         i = 0;
         while(i < _renderers.length)
         {
            renderer = _renderers[i];
            populateRendererData(renderer,i + this._startIndex);
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
            container.addChild(this.buttons_background);
            container.addChild(this.btn_prev);
            container.addChild(this.btn_next);
            this.buttons_background.visible = true;
            this.btn_prev.visible = true;
            this.btn_next.visible = true;
            this.btn_prev.enabled = this._startIndex > 0;
            this.btn_next.enabled = this._startIndex < _dataProvider.length - overflowIndex;
            this.btn_prev.x = offsetX + 8;
            this.btn_next.x = this.btn_prev.x + this.btn_next.width + this.btn_prev.width - 2;
            this.buttons_background.x = this.btn_prev.x - 7;
         }
         else
         {
            this.btn_prev.x = 0;
            this.btn_next.x = this.btn_prev.x + this.btn_next.width + this.btn_prev.width;
            this.btn_prev.visible = false;
            this.btn_next.visible = false;
            this.btn_prev.enabled = false;
            this.btn_next.enabled = false;
            this.buttons_background.visible = false;
            this.buttons_background.x = 0;
         }
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         dispatchEvent(new Event(Event.RESIZE));
      }
   }
}

