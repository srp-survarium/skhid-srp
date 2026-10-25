package survarium.controls
{
   import flash.display.MovieClip;
   import flash.filters.DropShadowFilter;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.utils.Dictionary;
   import flash.utils.getDefinitionByName;
   
   public class HUDIconLayer extends MovieClip
   {
      
      protected static const to_dir:Number = 180 / Math.PI;
      
      protected var _markers:Dictionary = new Dictionary();
      
      protected var _timeInterval:uint = 500;
      
      protected var _class:Dictionary = new Dictionary();
      
      protected var _enemyIds:Array = [];
      
      protected var _playerTeam:uint = 0;
      
      public function HUDIconLayer()
      {
         super();
      }
      
      public function register_icon_renderer(kind:uint, renderer_class:Class) : void
      {
         this._class[kind] = renderer_class;
      }
      
      public function create_player_icon(id:uint, kind:uint, baseScale:Number) : void
      {
         var renderer:HUDMarker = new this._class[kind]() as HUDMarker;
         renderer.baseScale = baseScale;
         if(kind == 0)
         {
            (renderer.textField as TextField).autoSize = TextFieldAutoSize.CENTER;
            renderer.textField.visible = false;
         }
         renderer.kind = kind;
         this._markers[id] = renderer;
         renderer.visible = false;
         addChild(renderer);
      }
      
      public function set_enemy_icon_ids(value:Array) : void
      {
         trace("set_player_icon_ids::",value,"length::",value.length);
         _enemyIds = value;
      }
      
      public function set_player_team(team:uint) : void
      {
         _playerTeam = team;
      }
      
      public function set_player_icon_team(id:uint, team:uint) : void
      {
         this._markers[id].team = team;
      }
      
      public function set_player_icon_name(id:uint, str:String) : void
      {
         this._markers[id].textField.text = str;
      }
      
      public function set_player_icon_type(id:uint, type:uint) : void
      {
         this._markers[id].type = type;
      }
      
      public function set_player_icon_container(id:uint, container:Boolean) : void
      {
         this._markers[id].container = container;
      }
      
      public function set_player_icon_rotation(id:uint, rot:Number) : void
      {
         this._markers[id].rotation = rot * to_dir;
      }
      
      public function set_player_icon_label(id:uint, label:String) : void
      {
         this._markers[id].label = label;
      }
      
      public function set_player_icon_glow(id:uint, bool:Boolean) : void
      {
         this._markers[id].glow = bool;
      }
      
      public function set_player_time_interval(value:uint) : void
      {
         var _marker:HUDMarker;
         for each(_marker in this._markers)
         {
            _marker.delay = value;
         }
      }
      
      public function set_player_icon_pos(id:uint, posX:Number, posY:Number, scale:Number) : void
      {
         var renderer:HUDMarker = this._markers[id];
         if(!renderer)
         {
            return;
         }
         renderer.x = posX;
         renderer.y = posY;
         renderer.scaleX = renderer.scaleY = scale * renderer.baseScale;
      }
      
      public function set_player_icon_visible(id:uint, bool:Boolean) : void
      {
         var canBeVisible:Boolean;
         if(this._markers[id])
         {
            if(bool)
            {
               trace("set_player_icon_visible:: kind:",this._markers[id].kind,"indexInEnemys:",this._enemyIds.indexOf(id));
               canBeVisible = this._markers[id].kind != 0 || this._enemyIds.indexOf(id) == -1;
               if(canBeVisible)
               {
                  this._markers[id].visible = true;
               }
            }
            else
            {
               this._markers[id].visible = false;
            }
         }
      }
      
      public function set_player_icon_props(props:Object) : void
      {
         var id:int = (props.id);
         var renderer:HUDMarker = this._markers[id];
         trace("set_player_icon_props",id,renderer);
         if(!renderer)
         {
            return;
         }
         var iconVisible:Boolean = props.visible == true || props.visible == undefined;
         var canBeVisible:Boolean;
         var textField:TextField;
         var newName:String;
         var colorTransfor:ColorTransform;
         var color:uint;
         var filter:DropShadowFilter;
         if(iconVisible)
         {
            canBeVisible = renderer.kind != 0 || this._enemyIds.indexOf(id) == -1;
            trace("canBeVisible",canBeVisible,renderer.kind,props.a);
            if(canBeVisible)
            {
            if(props.visible != undefined)
            {
               renderer.visible = props.visible;
            }
            if(props.x != undefined)
            {
               renderer.x = props.x;
               renderer.y = props.y;
               renderer.scaleX = renderer.scaleY = props.z * renderer.baseScale;
            }
            }
            else
            {
               return;
            }
            textField = renderer.textField;
            trace("textField",textField);
            if(textField)
            {
               if(props.name != undefined)
               {
                  newName = props.name;
                  if(textField.text != newName)
                  {
                     textField.text = newName;
                  }
               }
               if(props.modifier_visible != undefined)
               {
                  renderer.container = props.modifier_visible;
               }
               if(props.name_visible != undefined)
               {
                  if(props.name_visible)
                  {
                     renderer.showName();
                  }
                  else
                  {
                     renderer.hideName();
                  }
               }
            }
            if(props.rgb != undefined)
            {
               colorTransfor = new ColorTransform();
               color = (props.rgb);
               colorTransfor.color = color;
               if(renderer.icon.renderer)
               {
                  renderer.icon.renderer.transform.colorTransform = colorTransfor;
               }
               if(textField)
               {
                  textField.textColor = color;
                  if(textField.filters)
                  {
                     filter = textField.filters[0];
                     filter.color = color;
                  }
               }
            }
            if(props.a != undefined)
            {
               renderer.alpha = props.a / 255;
            }
         }
         else
         {
            renderer.visible = false;
         }
      }
      
      public function set_player_name_visible(id:uint, bool:Boolean) : void
      {
         var mk:HUDMarker = this._markers[id];
         if(!mk)
         {
            return;
         }
         if(bool)
         {
            mk.showName();
         }
         else
         {
            mk.hideName();
         }
      }
      
      public function set_player_icon_alpha(id:uint, alpha:uint) : void
      {
         var renderer:HUDMarker = this._markers[id];
         if(renderer)
         {
            renderer.alpha = alpha / 255;
         }
      }
      
      public function set_player_color(id:uint, red:uint, green:uint, blue:uint, alpha:uint = 255) : void
      {
         var color:uint = (red << 16 | green << 8 | blue);
         var colorTransfor:ColorTransform = new ColorTransform();
         colorTransfor.color = color;
         colorTransfor.alphaOffset = alpha - 255;
         var renderer:HUDMarker = this._markers[id];
         var filter:DropShadowFilter;
         if(renderer.icon.renderer)
         {
            renderer.icon.renderer.transform.colorTransform = colorTransfor;
         }
         if(renderer.textField)
         {
            renderer.textField.textColor = color;
         }
         if(renderer.textField && ((renderer.textField as TextField).filters))
         {
            filter = (renderer.textField as TextField).filters[0];
            filter.color = red << 16 | green << 8 | blue;
         }
      }
   }
}

